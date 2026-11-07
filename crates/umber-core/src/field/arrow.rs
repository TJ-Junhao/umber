//! The shape of an arrow: a filled shaft and head whose sizes do not follow its length.
//!
//! A shaft 1 pt wide ends in a head 5 pt long and 4 pt wide, whatever the arrow's length, so a
//! page of arrows looks even. The length is exact: an arrow shorter than its head has no shaft
//! and shrinks as a whole, its head scaled to its length, so that it is never lengthened. A
//! zero vector is a dot as wide as the shaft: the sample exists, and nothing moves there.

use super::{Reach, SCREEN_SCALES, direction, magnitude};
use crate::{ArrowLength, Pivot};

/// The width of a shaft, in points, and the diameter of the dot of a zero vector.
pub(crate) const SHAFT_WIDTH: f64 = 1.0;
/// The length of a head, in points.
pub(crate) const HEAD_LENGTH: f64 = 5.0;
/// The width of a head at its base, in points.
pub(crate) const HEAD_WIDTH: f64 = 4.0;

/// How the arrows of one mark are measured on the plot. A layout reads it to hold the arrows and
/// to size the key, and each layout of a figure with arrows leaves the next one a new frame
/// ([`crate::pipeline`]), so that limits, arrows and key always read the same numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ArrowFrame {
    /// Points per unit of magnitude; 0 for an automatic scale until a plot has been measured.
    pub pt_per_unit: f64,
    /// The points of one data unit along x and along y, of which only the ratio counts:
    /// [`SCREEN_SCALES`] but for [`Direction::Data`](crate::Direction::Data).
    pub scales: [f64; 2],
    /// The length of the key arrow, in points; 0 until a plot has been measured, so that the
    /// first layout has the most room to compare a key with.
    pub key_length: f64,
}

impl ArrowFrame {
    /// The frame of an arrow of `length` before anything is measured.
    pub(crate) fn of(length: ArrowLength) -> ArrowFrame {
        ArrowFrame {
            pt_per_unit: match length {
                ArrowLength::Auto => 0.0,
                ArrowLength::PtPerUnit(scale) => scale,
            },
            scales: SCREEN_SCALES,
            key_length: 0.0,
        }
    }

    /// The outline of the arrow of `(u, v)` in this frame; `None` for a zero vector, which is
    /// a dot.
    pub(crate) fn outline(&self, u: f64, v: f64, pivot: Pivot) -> Option<Outline> {
        let length = magnitude(u, v);
        (length != 0.0).then(|| {
            Outline::new(
                self.pt_per_unit * length,
                direction(u, v, self.scales),
                pivot,
            )
        })
    }

    /// How far the arrow of `(u, v)` reaches from its sample in this frame.
    pub(crate) fn reach(&self, u: f64, v: f64, pivot: Pivot) -> Reach {
        self.outline(u, v, pivot)
            .map_or(Reach::around(SHAFT_WIDTH / 2.0), |outline| outline.reach())
    }
}

/// Which end of the bracket of a [`ScaleSearch`] moved last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    /// The lower end.
    Low,
    /// The upper end.
    High,
}

/// The search for the automatic scale of one axes: the scale `k` whose layout measures `k`.
///
/// A longer scale reaches further, which widens the limits or takes room from the plot, so the
/// scale `g(k)` that a layout of `k` measures falls as `k` rises, and `g(k) = k` has one
/// solution. Following `g` alone reaches it when an arrow takes a small share of the plot, but
/// goes back and forth for ever when a few samples make the arrows long. So the search keeps a
/// bracket: the largest scale that measured longer and the smallest that measured shorter or
/// could not be laid out. It tries the point where a straight line between the two ends meets
/// `g(k) = k`, the false position, which for the nearly straight `g` of a field takes a step or
/// two, and halves the weight of an end that stays, so that the bracket closes from both sides
/// (the Illinois method). A scale that cannot be laid out has no measure, and the bracket is
/// bisected, so the search ends on the longest scale the plot can hold when no solution fits.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct ScaleSearch {
    /// The largest scale that measured longer, and by how much.
    low: Option<(f64, f64)>,
    /// The smallest scale that measured shorter, and by how much; no measure for a scale that
    /// could not be laid out.
    high: Option<(f64, Option<f64>)>,
    /// Which end moved last.
    moved: Option<Side>,
}

impl ScaleSearch {
    /// The scale to try after a layout of `held` measured `measured`.
    pub(crate) fn measured(&mut self, held: f64, measured: f64) -> f64 {
        if measured > held {
            let side = Side::Low;
            if self.moved == Some(side)
                && let Some((_, Some(gap))) = &mut self.high
            {
                *gap /= 2.0;
            }
            self.low = Some((held, measured - held));
            self.moved = Some(side);
        } else {
            let side = Side::High;
            if self.moved == Some(side)
                && let Some((_, gap)) = &mut self.low
            {
                *gap /= 2.0;
            }
            self.high = Some((held, Some(held - measured)));
            self.moved = Some(side);
        }
        self.next().unwrap_or(measured)
    }

    /// The scale to try after `held` could not be laid out.
    pub(crate) fn failed(&mut self, held: f64) -> f64 {
        self.high = Some((held, None));
        self.moved = Some(Side::High);
        self.next().unwrap_or(held / 2.0)
    }

    /// The point inside the bracket to try, once it has both ends.
    fn next(&self) -> Option<f64> {
        let ((low, low_gap), (high, high_gap)) = self.low.zip(self.high)?;
        Some(match high_gap {
            Some(high_gap) if low_gap + high_gap > 0.0 => {
                low + (high - low) * (low_gap / (low_gap + high_gap))
            }
            _ => low + (high - low) / 2.0,
        })
    }
}

/// The outline of an arrow, in points relative to its sample, y up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Outline {
    /// The corners, in order around the arrow; only the first `count` are used.
    corners: [[f64; 2]; 7],
    /// How many corners the outline has: 7 with a shaft, 3 without.
    count: usize,
}

impl Outline {
    /// The outline of an arrow `length` pt long along the unit vector `direction`, the part
    /// `pivot` names sitting on the sample.
    pub(crate) fn new(length: f64, [dx, dy]: [f64; 2], pivot: Pivot) -> Outline {
        let start = match pivot {
            Pivot::Tail => 0.0,
            Pivot::Middle => -length / 2.0,
            Pivot::Tip => -length,
        };
        // A position `along` the arrow from its tail and `across` it, to the left of it.
        let at = |along: f64, across: f64| {
            let along = start + along;
            [along * dx - across * dy, along * dy + across * dx]
        };
        if length < HEAD_LENGTH {
            let half = HEAD_WIDTH / 2.0 * (length / HEAD_LENGTH);
            let corners = [at(0.0, half), at(length, 0.0), at(0.0, -half)];
            let mut all = [[0.0; 2]; 7];
            all[..3].copy_from_slice(&corners);
            return Outline {
                corners: all,
                count: 3,
            };
        }
        let (neck, shaft, head) = (length - HEAD_LENGTH, SHAFT_WIDTH / 2.0, HEAD_WIDTH / 2.0);
        Outline {
            corners: [
                at(0.0, shaft),
                at(neck, shaft),
                at(neck, head),
                at(length, 0.0),
                at(neck, -head),
                at(neck, -shaft),
                at(0.0, -shaft),
            ],
            count: 7,
        }
    }

    /// The corners, in order around the arrow.
    pub(crate) fn corners(&self) -> &[[f64; 2]] {
        &self.corners[..self.count]
    }

    /// How far the arrow reaches from its sample.
    pub(crate) fn reach(&self) -> Reach {
        self.corners()
            .iter()
            .fold(Reach::default(), |reach, corner| reach.holding(*corner))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scale a search settles on for the measure `g`, and how many layouts that takes; `g`
    /// gives `None` for a scale that cannot be laid out.
    fn solve(g: impl Fn(f64) -> Option<f64>) -> (f64, usize) {
        let mut search = ScaleSearch::default();
        let (mut held, mut good) = (0.0, 0.0);
        for layouts in 1..=16 {
            match g(held) {
                Some(measured) => {
                    good = held;
                    if (measured - held).abs() <= 1e-6 * measured {
                        return (held, layouts);
                    }
                    held = search.measured(held, measured);
                }
                None => held = search.failed(held),
            }
        }
        (good, 16)
    }

    #[test]
    fn a_small_reach_settles_in_a_few_layouts() {
        // Twenty columns: the arrows take a twentieth of the plot.
        let (scale, layouts) = solve(|k| Some(20.0 * (1.0 - k / 400.0)));
        assert!((scale - 20.0 / 1.05).abs() < 1e-4, "{scale}");
        assert!(layouts <= 5, "{layouts}");
    }

    #[test]
    fn a_long_arrow_settles_where_following_the_measure_would_not() {
        // Slope -2: each step of `g` alone would double the distance to the solution.
        let (scale, layouts) = solve(|k| Some(120.0 - 2.0 * k));
        assert!((scale - 40.0).abs() < 1e-3, "{scale}");
        assert!(layouts <= 8, "{layouts}");
    }

    #[test]
    fn a_curved_measure_closes_the_bracket_from_both_sides() {
        let (scale, layouts) = solve(|k| Some(100.0 / (1.0 + k * k / 100.0)));
        assert!((100.0 / (1.0 + scale * scale / 100.0) - scale).abs() < 1e-3);
        assert!(layouts <= 12, "{layouts}");
    }

    #[test]
    fn a_scale_that_cannot_be_laid_out_is_bisected_away() {
        // The solution, 80, lies past what the plot holds: the search ends close below 50.
        let (scale, _) = solve(|k| (k < 50.0).then_some(80.0));
        assert!(scale < 50.0 && scale > 49.9, "{scale}");
    }

    #[test]
    fn a_head_keeps_its_size_at_any_length() {
        for length in [5.0, 20.0, 300.0] {
            let outline = Outline::new(length, [1.0, 0.0], Pivot::Tail);
            let corners = outline.corners();
            assert_eq!(corners.len(), 7);
            assert_eq!(corners[3], [length, 0.0]);
            assert_eq!(corners[2], [length - 5.0, 2.0]);
            assert_eq!(corners[4], [length - 5.0, -2.0]);
            assert_eq!((corners[0][1], corners[6][1]), (0.5, -0.5));
        }
    }

    #[test]
    fn an_arrow_shorter_than_its_head_shrinks_and_keeps_its_length() {
        let outline = Outline::new(2.5, [1.0, 0.0], Pivot::Tail);
        assert_eq!(outline.corners(), [[0.0, 1.0], [2.5, 0.0], [0.0, -1.0]]);
    }

    #[test]
    fn the_pivot_is_the_part_on_the_sample() {
        let tip = |pivot| Outline::new(10.0, [0.0, 1.0], pivot).corners()[3];
        assert_eq!(tip(Pivot::Tail), [0.0, 10.0]);
        assert_eq!(tip(Pivot::Middle), [0.0, 5.0]);
        assert_eq!(tip(Pivot::Tip), [0.0, 0.0]);
    }

    #[test]
    fn the_reach_follows_the_corners() {
        let reach = Outline::new(10.0, [1.0, 0.0], Pivot::Middle).reach();
        assert_eq!(
            reach,
            Reach {
                left: 5.0,
                right: 5.0,
                down: 2.0,
                up: 2.0
            }
        );
    }
}
