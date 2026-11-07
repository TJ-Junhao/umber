//! Vector fields: the geometry that arrows and wind barbs share, independent of any axes.
//!
//! Everything here works in points, with y pointing up, relative to the sample a shape is drawn
//! for; [`crate::draw`] places the shapes on the plot and [`crate::bubble_fit`] reads how far
//! they reach.
//!
//! Nothing here calls `sin`, `cos`, `atan2` or `hypot`. Those come from the platform's math
//! library, whose results differ in the last bits from one platform to the next, and a shape
//! must be the same everywhere. A direction is the vector itself over its length, and a length
//! is the square root of a sum of squares: addition, multiplication, division and the square
//! root are correctly rounded by IEEE 754, so they give the same bits on every platform.

pub(crate) mod arrow;
pub(crate) mod barb;
pub(crate) mod spacing;

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

/// The unit vector on the plot of a nonzero `(u, v)`, with one data unit along x and along y
/// `scales` points long, of which only the ratio counts. `(u, v)` is first divided by its
/// larger component, so that no product overflows.
pub(crate) fn direction(u: f64, v: f64, [x_scale, y_scale]: [f64; 2]) -> [f64; 2] {
    let largest = u.abs().max(v.abs());
    let (p, q) = (u / largest * x_scale, v / largest * y_scale);
    let length = magnitude(p, q);
    if length == 0.0 {
        // The scales differ so much that the product vanished: the vector as it is.
        let length = magnitude(u / largest, v / largest);
        return [u / largest / length, v / largest / length];
    }
    [p / length, q / length]
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
