//! Vector fields: the geometry that arrows and wind barbs share, independent of any axes, and
//! the streamlines traced through a grid ([`stream`]).
//!
//! Arrows and barbs work in points, with y pointing up, relative to the sample a shape is drawn
//! for; [`crate::draw`] places the shapes on the plot and [`crate::bubble_fit`] reads how far
//! they reach. Streamlines work in data coordinates and in steps of the grid.
//!
//! Nothing here calls `sin`, `cos`, `atan2` or `hypot`. Those come from the platform's math
//! library, whose results differ in the last bits from one platform to the next, and a shape
//! must be the same everywhere. A direction is the vector itself over its length, and a length
//! is the square root of a sum of squares: addition, multiplication, division and the square
//! root are correctly rounded by IEEE 754, so they give the same bits on every platform.

pub(crate) mod arrow;
pub(crate) mod barb;
pub(crate) mod spacing;
pub(crate) mod stream;

/// The size of the components beyond which their squares would overflow or underflow: `2^500`.
const SAFE_SQUARE: f64 = f64::from_bits((1023 + 500) << 52);
/// The power of two that brings large components into range: `2^-600`.
const SHRINK: f64 = f64::from_bits((1023 - 600) << 52);
/// The power of two that brings small components into range: `2^600`.
const GROW: f64 = f64::from_bits((1023 + 600) << 52);

/// The length of the vector `(u, v)`: `sqrt(u² + v²)`, with both components first scaled by a
/// power of two when their squares would leave the range of `f64`, which is exact. It stands in
/// for `hypot`, whose result depends on the platform: see the module documentation.
///
/// NaN when either component is NaN.
pub(crate) fn magnitude(u: f64, v: f64) -> f64 {
    let largest = u.abs().max(v.abs());
    let length = |u: f64, v: f64| (u * u + v * v).sqrt();
    if largest > SAFE_SQUARE {
        length(u * SHRINK, v * SHRINK) * GROW
    } else if largest != 0.0 && largest < 1.0 / SAFE_SQUARE {
        length(u * GROW, v * GROW) * SHRINK
    } else {
        length(u, v)
    }
}

/// How the data units of the two axes compare on the plot, as far as a direction reads them.
/// `[1.0, 1.0]` draws a vector as it is, with `u` right and `v` up.
pub(crate) const SCREEN_SCALES: [f64; 2] = [1.0, 1.0];

/// `x`, finite and not 0, as a mantissa from 1 to 2 in magnitude, with its sign, and the power of
/// two that it is multiplied by. Both are exact.
fn split(x: f64) -> (f64, i32) {
    const EXPONENT: u64 = 0x7ff << 52;
    // A subnormal number first moves up to where its bits have the normal form.
    let (x, shift) = if x.abs() < f64::MIN_POSITIVE {
        (x * GROW, -600)
    } else {
        (x, 0)
    };
    let bits = x.to_bits();
    let exponent = ((bits & EXPONENT) >> 52) as i32 - 1023;
    let mantissa = f64::from_bits((bits & !EXPONENT) | (1023 << 52));
    (mantissa, exponent + shift)
}

/// `2^exponent` for an exponent of 0 or less; 0 below what a normal number holds.
fn power_of_two(exponent: i32) -> f64 {
    if exponent < -1022 {
        0.0
    } else {
        f64::from_bits(((exponent + 1023) as u64) << 52)
    }
}

/// A finite number as a mantissa and a power of two, `mantissa × 2^power`, whose power is an
/// integer of any size: products and quotients of such numbers never overflow or underflow, and
/// each rounds its mantissa once. Zero has a zero mantissa and the lowest power, so that it never
/// sets the scale of a sum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Wide {
    /// The mantissa: 0, or from 1 to 4 in magnitude, with the sign of the number.
    mantissa: f64,
    /// The power of two.
    power: i32,
}

impl Wide {
    /// Zero.
    pub(crate) const ZERO: Wide = Wide {
        mantissa: 0.0,
        power: i32::MIN,
    };

    /// The finite `x`, exactly.
    pub(crate) fn of(x: f64) -> Wide {
        if x == 0.0 {
            return Wide::ZERO;
        }
        let (mantissa, power) = split(x);
        Wide { mantissa, power }
    }

    /// Whether this is zero.
    pub(crate) fn is_zero(self) -> bool {
        self.mantissa == 0.0
    }

    /// This number times `other`.
    pub(crate) fn times(self, other: Wide) -> Wide {
        if self.is_zero() || other.is_zero() {
            return Wide::ZERO;
        }
        Wide {
            mantissa: self.mantissa * other.mantissa,
            power: self.power.saturating_add(other.power),
        }
        .normal()
    }

    /// This number over the nonzero `other`.
    pub(crate) fn over(self, other: Wide) -> Wide {
        if self.is_zero() {
            return Wide::ZERO;
        }
        Wide {
            mantissa: self.mantissa / other.mantissa,
            power: self.power.saturating_sub(other.power),
        }
        .normal()
    }

    /// The nonzero number `mantissa × 2^power`, exactly.
    pub(crate) fn from_parts(mantissa: f64, power: i32) -> Wide {
        Wide { mantissa, power }.normal()
    }

    /// The mantissa, from 1 to 2 in magnitude, and the power of two of this nonzero number.
    pub(crate) fn parts(self) -> (f64, i32) {
        let Wide { mantissa, power } = self.normal();
        (mantissa, power)
    }

    /// The same number with a mantissa from 1 to 2 in magnitude, exactly.
    fn normal(self) -> Wide {
        let (mantissa, shift) = split(self.mantissa);
        Wide {
            mantissa,
            power: self.power.saturating_add(shift),
        }
    }

    /// The number as an `f64`: 0 below the smallest subnormal, infinite above the largest
    /// number.
    pub(crate) fn value(self) -> f64 {
        if self.is_zero() {
            return 0.0;
        }
        // Two exact powers of two, so that only the last product rounds.
        let power = self.power.clamp(-2200, 2200);
        let half = power / 2;
        self.mantissa * any_power_of_two(half) * any_power_of_two(power - half)
    }

    /// Whether this number, not negative, is at most `other`, not negative.
    pub(crate) fn at_most(self, other: Wide) -> bool {
        match (self.is_zero(), other.is_zero()) {
            (true, _) => true,
            (false, true) => false,
            (false, false) => {
                let (a, b) = (self.normal(), other.normal());
                (a.power, a.mantissa) <= (b.power, b.mantissa)
            }
        }
    }
}

/// `2^exponent` for an exponent from -1100 to 1100: 0 or infinite where `f64` holds no such
/// power, exact otherwise.
fn any_power_of_two(exponent: i32) -> f64 {
    if exponent > 1023 {
        f64::INFINITY
    } else if exponent >= -1022 {
        f64::from_bits(((exponent + 1023) as u64) << 52)
    } else if exponent >= -1074 {
        f64::from_bits(1 << (exponent + 1074))
    } else {
        0.0
    }
}

/// The components `p` and `q` brought to one power of two, that of the larger, as `f64`s, with
/// that power: a component more than 2^1022 times smaller than the other becomes 0, past
/// anything a direction or a length can show.
fn common(p: Wide, q: Wide) -> (f64, f64, i32) {
    let top = p.power.max(q.power);
    (
        p.mantissa * power_of_two(p.power.saturating_sub(top)),
        q.mantissa * power_of_two(q.power.saturating_sub(top)),
        top,
    )
}

/// The unit vector of the vector `(p, q)`, not zero, however large or small its components.
pub(crate) fn unit(p: Wide, q: Wide) -> [f64; 2] {
    let (p, q, _) = common(p, q);
    let length = (p * p + q * q).sqrt();
    [p / length, q / length]
}

/// The length of the vector `(p, q)`, however large or small its components.
pub(crate) fn length(p: Wide, q: Wide) -> Wide {
    if p.is_zero() && q.is_zero() {
        return Wide::ZERO;
    }
    let (p, q, top) = common(p, q);
    Wide {
        mantissa: (p * p + q * q).sqrt(),
        power: top,
    }
}

/// The unit vector on the plot of a nonzero `(u, v)`, with one data unit along x and along y
/// `scales` points long, of which only the ratio counts.
///
/// Each component is multiplied by its scale as a mantissa and a power of two ([`Wide`]), and
/// the two products are brought together by exact powers of two, so that nothing overflows or
/// underflows on the way: `u` of 1e200 on a scale of 1e-198 and `v` of 1e-200 on a scale of
/// 1e202 make a vector of ordinary size, whatever the sizes of the parts. A product that
/// underflows at the end is below 2^-1022 of the other, past anything a direction can show.
pub(crate) fn direction(u: f64, v: f64, [x_scale, y_scale]: [f64; 2]) -> [f64; 2] {
    let product = |component: f64, scale: f64| {
        if component == 0.0 || scale == 0.0 {
            return Wide::ZERO;
        }
        let ((a, a_power), (b, b_power)) = (split(component), split(scale));
        Wide {
            mantissa: a * b,
            power: a_power + b_power,
        }
    };
    unit(product(u, x_scale), product(v, y_scale))
}

/// How far a shape reaches from its sample towards each side of the plot, in points; each at
/// least 0.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Reach {
    /// Towards the low end of the x axis.
    pub left: f64,
    /// Towards the high end of the x axis.
    pub right: f64,
    /// Towards the low end of the y axis.
    pub down: f64,
    /// Towards the high end of the y axis.
    pub up: f64,
}

impl Reach {
    /// The same distance towards every side.
    pub(crate) fn around(distance: f64) -> Reach {
        Reach {
            left: distance,
            right: distance,
            down: distance,
            up: distance,
        }
    }

    /// This reach, grown to hold `point`, relative to the sample, y up.
    pub(crate) fn holding(self, [x, y]: [f64; 2]) -> Reach {
        Reach {
            left: self.left.max(-x),
            right: self.right.max(x),
            down: self.down.max(-y),
            up: self.up.max(y),
        }
    }

    /// This reach with `margin` added towards every side.
    pub(crate) fn widened(self, margin: f64) -> Reach {
        Reach {
            left: self.left + margin,
            right: self.right + margin,
            down: self.down + margin,
            up: self.up + margin,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_magnitude_is_exact_where_the_squares_are() {
        assert_eq!(magnitude(3.0, 4.0), 5.0);
        assert_eq!(magnitude(-3.5, 12.0), 12.5);
        assert_eq!(magnitude(1.5, -2.0), 2.5);
        assert_eq!(magnitude(0.0, 0.0), 0.0);
        assert!(magnitude(f64::NAN, 1.0).is_nan());
    }

    #[test]
    fn the_magnitude_stays_finite_at_the_ends_of_the_range() {
        assert!((magnitude(3e300, 4e300) / 5e300 - 1.0).abs() < 1e-15);
        assert_eq!(magnitude(0.0, 1e300), 1e300);
        assert!((magnitude(3e-300, 4e-300) / 5e-300 - 1.0).abs() < 1e-15);
        // The power-of-two constants are what their names say.
        assert_eq!(SAFE_SQUARE, 2.0_f64.powi(500));
        assert_eq!(SHRINK, 2.0_f64.powi(-600));
        assert_eq!(GROW, 2.0_f64.powi(600));
    }

    #[test]
    fn a_direction_is_the_vector_over_its_length() {
        assert_eq!(direction(3.0, 4.0, SCREEN_SCALES), [0.6, 0.8]);
        assert_eq!(direction(-2.0, 0.0, SCREEN_SCALES), [-1.0, 0.0]);
        // One unit of x as long as four of y turns (1, 1) towards x.
        let [x, y] = direction(1.0, 1.0, [4.0, 1.0]);
        assert!((x / y - 4.0).abs() < 1e-12);
        assert_eq!(direction(0.0, 1e-300, [1.0, 1e-320]), [0.0, 1.0]);
    }

    #[test]
    fn a_direction_keeps_components_that_compensate_across_any_range() {
        // Limits 0 to 1e200 and 0 to 1e-200 on a plot of 400 by 240 pt.
        let [x, y] = direction(1e200, 1e-200, [400.0 / 1e200, 240.0 / 1e-200]);
        assert!((y / x - 0.6).abs() < 1e-12, "{x} {y}");
        let [x, y] = direction(3e-320, 4e-320, [1e300, 1e300]);
        assert!((x - 0.6).abs() < 1e-3 && (y - 0.8).abs() < 1e-3, "{x} {y}");
        assert_eq!(direction(1e300, 1e-300, [1e300, 1e-300]), [1.0, 0.0]);
    }

    #[test]
    fn wide_numbers_keep_any_size() {
        let huge = Wide::of(1e300).times(Wide::of(1e300));
        let tiny = Wide::of(1e-300).times(Wide::of(1e-300));
        assert_eq!(huge.over(Wide::of(1e300)).value(), 1e300);
        assert_eq!(tiny.over(Wide::of(1e-300)).value(), 1e-300);
        assert!((huge.times(tiny).value() - 1.0).abs() < 1e-15);
        assert_eq!(huge.value(), f64::INFINITY);
        assert_eq!(tiny.value(), 0.0);
        assert_eq!(
            Wide::of(3.0).over(Wide::of(f64::MAX)).value(),
            3.0 / f64::MAX
        );
        assert_eq!(
            Wide::of(1.5).times(Wide::of(2f64.powi(-1060))).value(),
            1.5 * 2f64.powi(-1060)
        );
        assert!(tiny.at_most(huge) && !huge.at_most(tiny) && Wide::ZERO.at_most(tiny));
        assert!(Wide::of(3.0).at_most(Wide::of(3.0)) && !Wide::of(3.5).at_most(Wide::of(3.0)));
        let big = Wide::of(2f64.powi(1000)).times(Wide::of(2f64.powi(1000)));
        let long = length(Wide::of(3.0).times(big), Wide::of(4.0).times(big));
        assert_eq!(long.over(big).value(), 5.0);
        assert_eq!(unit(huge, Wide::ZERO), [1.0, 0.0]);
    }

    #[test]
    fn a_split_is_exact() {
        for x in [1.0, -1.5, 3e-320, 1e300, -5e-324, f64::MIN_POSITIVE] {
            let (mantissa, power) = split(x);
            assert!((1.0..2.0).contains(&mantissa.abs()), "{x}");
            let halves = power.min(0).abs();
            let back = mantissa * 2.0_f64.powi(power + halves) / 2.0_f64.powi(halves / 2);
            assert_eq!(back / 2.0_f64.powi(halves - halves / 2), x);
        }
    }

    #[test]
    fn a_reach_grows_to_hold_points_and_never_goes_negative() {
        let reach = Reach::default().holding([3.0, -2.0]).holding([-1.0, -5.0]);
        assert_eq!(
            reach,
            Reach {
                left: 1.0,
                right: 3.0,
                down: 5.0,
                up: 0.0
            }
        );
        assert_eq!(reach.widened(1.0).down, 6.0);
        assert_eq!(Reach::around(2.0).right, 2.0);
    }
}
