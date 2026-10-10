//! An exact oracle for [`clip_segment`]: Liang–Barsky in exact dyadic and rational
//! arithmetic on a small big-integer type, so it shares no rounding with the code under
//! test.

use super::super::clip::{Bounds, clip_segment};
use super::super::mapping::{AxisMap, AxisMaps, AxisPoint, PlotPoint};
use std::cmp::Ordering;
use std::collections::BTreeMap;

/// A signed big integer: little-endian 64-bit limbs without trailing zeros.
#[derive(Clone, Debug, PartialEq, Eq)]
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
    let mut out = Vec::with_capacity(a.len().max(b.len()) + 1);
    let mut carry = 0_u128;
    for i in 0..a.len().max(b.len()) {
        let s = u128::from(*a.get(i).unwrap_or(&0)) + u128::from(*b.get(i).unwrap_or(&0)) + carry;
        out.push(s as u64);
        carry = s >> 64;
    }
    out.push(carry as u64);
    trim(out)
}

/// `a - b` for `|a| >= |b|`.
fn sub_mag(a: &[u64], b: &[u64]) -> Vec<u64> {
    let mut out = Vec::with_capacity(a.len());
    let mut borrow = 0_i128;
    for (i, &x) in a.iter().enumerate() {
        let mut d = i128::from(x) - i128::from(*b.get(i).unwrap_or(&0)) - borrow;
        borrow = 0;
        if d < 0 {
            d += 1_i128 << 64;
            borrow = 1;
        }
        out.push(d as u64);
    }
    trim(out)
}

fn mul_mag(a: &[u64], b: &[u64]) -> Vec<u64> {
    let mut out = vec![0_u64; a.len() + b.len()];
    for (i, &x) in a.iter().enumerate() {
        let mut carry = 0_u128;
        for (j, &y) in b.iter().enumerate() {
            let t = u128::from(x) * u128::from(y) + u128::from(out[i + j]) + carry;
            out[i + j] = t as u64;
            carry = t >> 64;
        }
        out[i + b.len()] = carry as u64;
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
    fn neg(&self) -> Big {
        Big::new(!self.neg, self.mag.clone())
    }
    fn mul(&self, o: &Big) -> Big {
        Big::new(self.neg != o.neg, mul_mag(&self.mag, &o.mag))
    }
    fn shl(&self, bits: u32) -> Big {
        Big::new(self.neg, shl_mag(&self.mag, bits))
    }
    fn signum(&self) -> i32 {
        if self.mag.is_empty() {
            0
        } else if self.neg {
            -1
        } else {
            1
        }
    }
    /// `|self|` as `(m, e)` with `|self| ≈ m * 2^e`.
    fn approx(&self) -> (f64, i64) {
        let Some(&top) = self.mag.last() else {
            return (0.0, 0);
        };
        let below = if self.mag.len() >= 2 {
            self.mag[self.mag.len() - 2]
        } else {
            0
        };
        let m = top as f64 * 2f64.powi(64) + below as f64;
        (m, 64 * (self.mag.len() as i64 - 2))
    }
}

/// An exact binary fraction `m * 2^e`.
#[derive(Clone, Debug)]
struct Dy {
    m: Big,
    e: i32,
}

impl Dy {
    fn from_f64(v: f64) -> Dy {
        assert!(v.is_finite());
        if v == 0.0 {
            return Dy {
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
        Dy {
            m: Big::new(v < 0.0, vec![m]),
            e,
        }
    }
    fn int(v: u64) -> Dy {
        Dy {
            m: Big::new(false, vec![v]),
            e: 0,
        }
    }
    fn align(&self, o: &Dy) -> (Big, Big, i32) {
        let e = self.e.min(o.e);
        (
            self.m.shl((self.e - e) as u32),
            o.m.shl((o.e - e) as u32),
            e,
        )
    }
    fn add(&self, o: &Dy) -> Dy {
        let (a, b, e) = self.align(o);
        Dy { m: a.add(&b), e }
    }
    fn sub(&self, o: &Dy) -> Dy {
        self.add(&o.neg())
    }
    fn neg(&self) -> Dy {
        Dy {
            m: self.m.neg(),
            e: self.e,
        }
    }
    fn mul(&self, o: &Dy) -> Dy {
        Dy {
            m: self.m.mul(&o.m),
            e: self.e + o.e,
        }
    }
    fn signum(&self) -> i32 {
        self.m.signum()
    }
    fn abs(&self) -> Dy {
        if self.signum() < 0 {
            self.neg()
        } else {
            self.clone()
        }
    }
    fn cmp(&self, o: &Dy) -> Ordering {
        self.sub(o).signum().cmp(&0)
    }
    /// `self / o` for positive values, approximately.
    fn ratio(&self, o: &Dy) -> f64 {
        let ((m1, e1), (m2, e2)) = (self.m.approx(), o.m.approx());
        if m1 == 0.0 {
            return 0.0;
        }
        let shift = (e1 + i64::from(self.e) - e2 - i64::from(o.e)).clamp(-2000, 2000);
        m1 / m2 * 2f64.powi(shift as i32)
    }
}

/// A rational `n / d` with `d > 0`.
#[derive(Clone, Debug)]
struct Q {
    n: Dy,
    d: Dy,
}

impl Q {
    fn lt(&self, o: &Q) -> bool {
        self.n.mul(&o.d).cmp(&o.n.mul(&self.d)) == Ordering::Less
    }
    fn eq(&self, o: &Q) -> bool {
        self.n.mul(&o.d).cmp(&o.n.mul(&self.d)) == Ordering::Equal
    }
}

/// The exact visible parameter interval of `a..b` in `bounds`.
fn exact(a: AxisPoint, b: AxisPoint, bounds: &Bounds) -> Option<(Q, Q)> {
    let f = Dy::from_f64;
    let (ax, ay) = (f(a.x), f(a.y));
    let (dx, dy) = (f(b.x).sub(&ax), f(b.y).sub(&ay));
    let edges = [
        (dx.neg(), ax.sub(&f(bounds.left))),
        (dx.clone(), f(bounds.right).sub(&ax)),
        (dy.neg(), ay.sub(&f(bounds.top))),
        (dy.clone(), f(bounds.bottom).sub(&ay)),
    ];
    let mut t0 = Q {
        n: Dy::int(0),
        d: Dy::int(1),
    };
    let mut t1 = Q {
        n: Dy::int(1),
        d: Dy::int(1),
    };
    for (p, q) in edges {
        match p.signum() {
            0 => {
                if q.signum() < 0 {
                    return None;
                }
            }
            s => {
                let t = if s < 0 {
                    Q {
                        n: q.neg(),
                        d: p.neg(),
                    }
                } else {
                    Q { n: q, d: p }
                };
                if s < 0 {
                    if t1.lt(&t) {
                        return None;
                    }
                    if t0.lt(&t) {
                        t0 = t;
                    }
                } else {
                    if t.lt(&t0) {
                        return None;
                    }
                    if t.lt(&t1) {
                        t1 = t;
                    }
                }
            }
        }
    }
    (!t1.lt(&t0)).then_some((t0, t1))
}

/// The error of `got` against the exact point at `t` on `a..b`, in points; `None`
/// when it exceeds 1e-3.
fn error(a: AxisPoint, b: AxisPoint, t: &Q, got: AxisPoint) -> Option<f64> {
    let f = Dy::from_f64;
    let mut worst = 0.0_f64;
    for (a, b, g) in [(a.x, b.x, got.x), (a.y, b.y, got.y)] {
        // got - (a + t (b - a)) = (got d - a d - n (b - a)) / d.
        let (a, d) = (f(a), f(b).sub(&f(a)));
        let diff = f(g).mul(&t.d).sub(&a.mul(&t.d)).sub(&t.n.mul(&d)).abs();
        if diff.mul(&Dy::int(1000)).cmp(&t.d) == Ordering::Greater {
            return None;
        }
        worst = worst.max(diff.ratio(&t.d));
    }
    Some(worst)
}

/// The exact visible part of `a..b` (in axis space) inside `bounds`, mapped to plot
/// coordinates with `maps`: the mapping starts from the exact rational point, so only
/// the final conversion to `f64` rounds.
pub(in super::super) fn exact_plot(
    a: AxisPoint,
    b: AxisPoint,
    bounds: &Bounds,
    maps: AxisMaps,
) -> Option<(PlotPoint, PlotPoint)> {
    let (t0, t1) = exact(a, b, bounds)?;
    let at = |t: &Q| PlotPoint {
        x: position(a.x, b.x, t, &maps.x),
        y: position(a.y, b.y, t, &maps.y),
    };
    Some((at(&t0), at(&t1)))
}

/// The plot coordinate of `a + t (b - a)` under `map`.
fn position(a: f64, b: f64, t: &Q, map: &AxisMap) -> f64 {
    let f = Dy::from_f64;
    // (a + t (b - a) - lo) / (hi - lo), with t = n / d.
    let num = f(a)
        .mul(&t.d)
        .add(&t.n.mul(&f(b).sub(&f(a))))
        .sub(&f(map.lo).mul(&t.d));
    let den = t.d.mul(&f(map.hi).sub(&f(map.lo)));
    let pos = if num.signum() < 0 {
        -num.abs().ratio(&den)
    } else {
        num.ratio(&den)
    };
    map.start + pos * map.extent
}

/// Runs `n` seeded random segments through [`clip_segment`] and the oracle; panics on
/// any disagreement and returns a summary.
pub(super) fn run(n: usize) -> String {
    let mut state = 151_010_u64;
    let mut rng = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 11) as f64 / (1_u64 << 53) as f64
    };
    let fixed = Bounds {
        left: 10.0,
        top: 20.0,
        right: 410.0,
        bottom: 260.0,
    };
    let corners = [(10.0, 20.0), (410.0, 20.0), (10.0, 260.0), (410.0, 260.0)];
    let mut counts: BTreeMap<&str, [usize; 3]> = BTreeMap::new();
    let (mut max_error, mut touches) = (0.0_f64, 0);
    for i in 0..n {
        let mut u = |lo: f64, hi: f64| lo + (hi - lo) * rng();
        let (tag, a, b): (&str, (f64, f64), (f64, f64)) = match i % 20 {
            0..=3 => (
                "ordinary",
                (u(-1000.0, 1000.0), u(-1000.0, 1000.0)),
                (u(-1000.0, 1000.0), u(-1000.0, 1000.0)),
            ),
            4..=7 => {
                let c = corners[(u(0.0, 4.0) as usize).min(3)];
                let (dx, dy) = (u(-1000.0, 1000.0).round(), u(-1000.0, 1000.0).round());
                ("corner", (c.0 - dx, c.1 - dy), (c.0 + dx, c.1 + dy))
            }
            8..=10 => {
                let (s, t) = (u(-1000.0, 1000.0), u(-1000.0, 1000.0));
                let edge = [10.0, 410.0, 20.0, 260.0][(u(0.0, 4.0) as usize).min(3)];
                if edge > 15.0 && edge < 300.0 && edge != 410.0 {
                    ("boundary", (s, edge), (t, edge))
                } else {
                    ("boundary", (edge, s), (edge, t))
                }
            }
            11 | 12 => {
                let p = (u(-1000.0, 1000.0), u(-1000.0, 1000.0));
                ("degenerate", p, p)
            }
            13..=15 => {
                let scale = 10f64.powf(u(9.0, 300.0));
                let (a, b) = match (u(0.0, 3.0) as usize).min(2) {
                    0 => {
                        let y = u(-100.0, 350.0);
                        ((-scale * u(0.5, 2.0), y), (scale * u(0.5, 2.0), y))
                    }
                    1 => {
                        let x = u(-100.0, 500.0);
                        ((x, -scale * u(0.5, 2.0)), (x, scale * u(0.5, 2.0)))
                    }
                    _ => ((-scale, -scale * u(0.5, 2.0)), (scale, scale * u(0.5, 2.0))),
                };
                if u(0.0, 1.0) < 0.5 {
                    ("huge", a, b)
                } else {
                    ("huge", b, a)
                }
            }
            16 | 17 => {
                let scale = 10f64.powf(u(12.0, 300.0));
                let far = (scale * u(-2.0, 2.0), scale * u(-2.0, 2.0));
                let inside = (u(10.0, 410.0), u(20.0, 260.0));
                if u(0.0, 1.0) < 0.5 {
                    ("huge_to_inside", far, inside)
                } else {
                    ("huge_to_inside", inside, far)
                }
            }
            18 => {
                // Both ends astronomically far, on a line exactly through the origin,
                // inside a clip rectangle around it.
                let scale = 10f64.powf(u(7.0, 307.0));
                let far = (scale * u(-1.0, 1.0), scale * u(-1.0, 1.0));
                ("huge_symmetric", far, (-far.0, -far.1))
            }
            _ => {
                // Both ends far, on a line through a point near the clip rectangle.
                let scale = 10f64.powf(u(7.0, 300.0));
                let c = (u(-50.0, 470.0), u(-50.0, 330.0));
                let angle = u(0.0, std::f64::consts::TAU);
                let (dx, dy) = (angle.cos(), angle.sin());
                let (s, t) = (scale * u(0.5, 2.0), scale * u(0.5, 2.0));
                (
                    "huge_through",
                    (c.0 - s * dx, c.1 - s * dy),
                    (c.0 + t * dx, c.1 + t * dy),
                )
            }
        };
        let (a, b) = (AxisPoint::new(a.0, a.1), AxisPoint::new(b.0, b.1));
        let bounds = if tag == "huge_symmetric" {
            Bounds {
                left: -u(1.0, 400.0),
                top: -u(1.0, 300.0),
                right: u(1.0, 400.0),
                bottom: u(1.0, 300.0),
            }
        } else {
            fixed
        };
        let got = clip_segment(a, b, &bounds);
        let want = exact(a, b, &bounds);
        let entry = counts.entry(tag).or_default();
        entry[0] += 1;
        match (want, got) {
            (None, None) => {}
            (Some((t0, t1)), Some((start, end))) => {
                entry[1] += 1;
                for p in [start, end] {
                    assert!(bounds.contains(p), "{tag} {a:?} {b:?}: {p:?} outside");
                    assert!(p.x.abs() <= 1e6 && p.y.abs() <= 1e6);
                }
                let e0 = error(a, b, &t0, start);
                let e1 = error(a, b, &t1, end);
                let (Some(e0), Some(e1)) = (e0, e1) else {
                    panic!("{tag} {a:?} {b:?}: got {start:?} {end:?}, error above 1e-3")
                };
                max_error = max_error.max(e0).max(e1);
            }
            (Some((t0, t1)), None) => {
                // Only a segment that merely touches the boundary may be missed.
                assert!(t0.eq(&t1), "{tag} {a:?} {b:?}: visible part lost");
                entry[2] += 1;
                touches += 1;
            }
            (None, Some((start, end))) => {
                // Only a rounding-level touch may be reported for a miss.
                assert_eq!(start, end, "{tag} {a:?} {b:?}: invisible part drawn");
                entry[2] += 1;
                touches += 1;
            }
        }
    }
    let mut summary = format!(
        "{n} segments, max intersection error {max_error:.3e} pt, {touches} \
         boundary touches resolved either way"
    );
    for (tag, [total, visible, touch]) in counts {
        summary.push_str(&format!(
            "\n  {tag}: {total} cases, {visible} visible, {touch} touches"
        ));
    }
    summary
}
