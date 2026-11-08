//! The shape of an arrow: a filled shaft and head that keep their size on a long arrow and
//! shrink with a short one.
//!
//! An arrow [`REFERENCE_LENGTH`] long or longer has a shaft 1 pt wide and a head 5 pt long and
//! 4 pt wide, whatever its length, so a page of long arrows looks even. A shorter arrow shrinks
//! as a whole by its length over [`REFERENCE_LENGTH`], head length, head width and shaft width
//! together, so its head is at most a third of it, and the length is exact: it is never
//! lengthened. The shrinking stops at a shaft [`MIN_SHAFT_WIDTH`] wide, one pixel at 144 dpi,
//! and at a head twice as wide as that shaft, so that the tiniest arrow still reads as an arrow
//! and its head is always wider than its shaft. A zero vector is a dot as wide as a full shaft:
//! the sample exists, and nothing moves there.

use super::{Reach, SCREEN_SCALES, direction, magnitude};
use crate::{ArrowLength, Direction, Pivot};

/// The width of a full-size shaft, in points, and the diameter of the dot of a zero vector.
pub(crate) const SHAFT_WIDTH: f64 = 1.0;
/// The length of a full-size head, in points.
pub(crate) const HEAD_LENGTH: f64 = 5.0;
/// The width of a full-size head at its base, in points.
pub(crate) const HEAD_WIDTH: f64 = 4.0;
/// The length, in points, from which an arrow has the full-size shaft and head.
pub(crate) const REFERENCE_LENGTH: f64 = 15.0;
/// The narrowest shaft, in points, of an arrow too short for its shaft to keep its proportion.
pub(crate) const MIN_SHAFT_WIDTH: f64 = 0.5;

/// How the arrows of one mark are measured on the plot. A layout reads it to hold the arrows and
/// to size the key, and each layout of a figure with arrows leaves the next one a new frame
/// ([`crate::pipeline`]), so that limits, arrows and key always read the same numbers.
///
/// A scale is kept as a length and the magnitude it belongs to, not as their quotient: an arrow
/// is that length times its share of that magnitude, so tiny magnitudes need no scale beyond the
/// range of `f64`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ArrowFrame {
    /// The length, in points, of an arrow of [`magnitude`](Self::magnitude); 0 until the axes
    /// are known, for an automatic scale and for arrows along the data.
    pub length: f64,
    /// The magnitude an arrow of [`length`](Self::length) has: the longest of an automatic scale,
    /// and 1 for a given number of points per unit. Greater than 0.
    pub magnitude: f64,
    /// The points of one data unit along x and along y, as they are, however far apart, of which
    /// only the ratio counts: [`SCREEN_SCALES`] but for [`Direction::Data`].
    pub scales: [f64; 2],
    /// The length of the key arrow, in points; 0 until a plot has been measured, so that the
    /// first layout has the most room to compare a key with.
    pub key_length: f64,
}

impl ArrowFrame {
    /// The frame of an arrow of `length` along `direction` before anything is measured. Arrows
    /// along the data have no direction yet, so they wait for the axes, as an automatic scale
    /// does; it is the layout without them that says how the axes compare.
    pub(crate) fn of(length: ArrowLength, direction: Direction) -> ArrowFrame {
        ArrowFrame {
            length: match (length, direction) {
                (ArrowLength::PtPerUnit(scale), Direction::Screen) => scale,
                (ArrowLength::PtPerUnit(_), Direction::Data) | (ArrowLength::Auto, _) => 0.0,
            },
            magnitude: 1.0,
            scales: SCREEN_SCALES,
            key_length: 0.0,
        }
    }

    /// The length, in points, of an arrow of `magnitude` in this frame: its share of
    /// [`self.magnitude`](Self::magnitude) of [`self.length`](Self::length).
    pub(crate) fn length_of(&self, magnitude: f64) -> f64 {
        if self.length == 0.0 {
            return 0.0;
        }
        self.length * (magnitude / self.magnitude)
    }

    /// The outline of the arrow of `(u, v)` in this frame; `None` for a zero vector, which is
    /// a dot.
    pub(crate) fn outline(&self, u: f64, v: f64, pivot: Pivot) -> Option<Outline> {
        let magnitude = magnitude(u, v);
        (magnitude != 0.0).then(|| {
            Outline::new(
                self.length_of(magnitude),
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

/// The widest bracket of a search for a ratio that false position is tried on, as high over low.
const NARROW_BRACKET: f64 = 2.0;
/// How far a search for a ratio moves past a ratio that could not be laid out, with no other end
/// to its bracket.
const RATIO_STEP: f64 = 4.0;

/// Which end of the bracket of a [`ScaleSearch`] moved last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    /// The lower end.
    Low,
    /// The upper end.
    High,
}

/// The search for a value `k` of a layout that the same layout measures: the length of the
/// longest automatic arrow of one axes, or the ratio of the scales of its plot.
///
/// A longer arrow reaches further, which widens the limits or takes room from the plot, so the
/// length `g(k)` that a layout of `k` measures falls as `k` rises, and `g(k) = k` has one
/// solution; so does a ratio, for the reasons of
/// [`settle_directions`](crate::pipeline). Following `g` alone reaches it when an arrow takes a
/// small share of the plot, but goes back and forth for ever when a few samples make the arrows
/// long. So the search keeps a bracket: the largest value that measured more and the smallest
/// that measured less, or that could not be laid out. It tries the point where a straight line
/// between the two ends meets `g(k) = k`, the false position, which for the nearly straight `g`
/// of a field takes a step or two, and halves the weight of an end that stays, so that the
/// bracket closes from both sides (the Illinois method). A value that cannot be laid out has no
/// measure, and the bracket is bisected, so the search ends on the longest arrow the plot can
/// hold when no solution fits.
///
/// A ratio can be anything from far below 1 to far above it, so while its bracket spans more than
/// a factor of [`NARROW_BRACKET`] it is bisected on a log scale, by the geometric mean: a square
/// root, which unlike a logarithm is the same on every platform.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct ScaleSearch {
    /// The largest value that measured more, and by how much; no measure for a ratio that could
    /// not be laid out.
    low: Option<(f64, Option<f64>)>,
    /// The smallest value that measured less, and by how much; no measure for a value that could
    /// not be laid out.
    high: Option<(f64, Option<f64>)>,
    /// Which end moved last.
    moved: Option<Side>,
    /// Whether this is a search for a ratio.
    ratio: bool,
}

impl ScaleSearch {
    /// The search for a ratio.
    pub(crate) fn for_ratio() -> ScaleSearch {
        ScaleSearch {
            ratio: true,
            ..ScaleSearch::default()
        }
    }

    /// The value to try after a layout of `held` measured `measured`.
    pub(crate) fn measured(&mut self, held: f64, measured: f64) -> f64 {
        if measured > held {
            if self.moved == Some(Side::Low)
                && let Some((_, Some(gap))) = &mut self.high
            {
                *gap /= 2.0;
            }
            self.low = Some((held, Some(measured - held)));
            self.moved = Some(Side::Low);
        } else {
            if self.moved == Some(Side::High)
                && let Some((_, Some(gap))) = &mut self.low
            {
                *gap /= 2.0;
            }
            self.high = Some((held, Some(held - measured)));
            self.moved = Some(Side::High);
        }
        self.next().unwrap_or(measured)
    }

    /// The length to try after `held` could not be laid out.
    pub(crate) fn failed(&mut self, held: f64) -> f64 {
        self.high = Some((held, None));
        self.moved = Some(Side::High);
        self.next().unwrap_or(held / 2.0)
    }

    /// The ratio to try after `held` could not be laid out: the arrows reach too far along x
    /// when `along_x`, and so the ratio is too high, and too far along y otherwise.
    pub(crate) fn failed_ratio(&mut self, held: f64, along_x: bool) -> f64 {
        let (end, side, step) = if along_x {
            (&mut self.high, Side::High, 1.0 / RATIO_STEP)
        } else {
            (&mut self.low, Side::Low, RATIO_STEP)
        };
        *end = Some((held, None));
        self.moved = Some(side);
        self.next().unwrap_or(held * step)
    }

    /// The point inside the bracket to try, once it has both ends.
    fn next(&self) -> Option<f64> {
        let ((low, low_gap), (high, high_gap)) = self.low.zip(self.high)?;
        if self.ratio && high > NARROW_BRACKET * low {
            return Some((low * high).sqrt());
        }
        Some(match low_gap.zip(high_gap) {
            Some((low_gap, high_gap)) if low_gap + high_gap > 0.0 => {
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
    /// `pivot` names sitting on the sample. An arrow shorter than [`REFERENCE_LENGTH`] has its
    /// head and shaft scaled to it (see the module documentation).
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
        let scale = (length / REFERENCE_LENGTH).min(1.0);
        let shaft = (SHAFT_WIDTH * scale).max(MIN_SHAFT_WIDTH) / 2.0;
        let head_length = HEAD_LENGTH * scale;
        let head = (HEAD_WIDTH * scale).max(4.0 * shaft) / 2.0;
        let neck = length - head_length;
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

    /// The head of an arrow alone, [`HEAD_LENGTH`] long and [`HEAD_WIDTH`] wide, centred on the
    /// point it is drawn for and pointing along the unit vector `direction`: the head a
    /// streamline carries.
    pub(crate) fn head([dx, dy]: [f64; 2]) -> Outline {
        let at = |along: f64, across: f64| [along * dx - across * dy, along * dy + across * dx];
        let (back, half) = (-HEAD_LENGTH / 2.0, HEAD_WIDTH / 2.0);
        let mut corners = [[0.0; 2]; 7];
        corners[..3].copy_from_slice(&[
            at(back, half),
            at(back + HEAD_LENGTH, 0.0),
            at(back, -half),
        ]);
        Outline { corners, count: 3 }
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

    /// The ratio a search settles on for the measure `g`, as [`solve`] does; `g` gives
    /// `Err(along_x)` for a ratio that cannot be laid out.
    fn solve_ratio(start: f64, g: impl Fn(f64) -> Result<f64, bool>) -> (f64, usize) {
        let mut search = ScaleSearch::for_ratio();
        let mut held = start;
        for layouts in 1..=12 {
            match g(held) {
                Ok(measured) if (measured - held).abs() <= 1e-6 * measured => {
                    return (held, layouts);
                }
                Ok(measured) => held = search.measured(held, measured),
                Err(along_x) => held = search.failed_ratio(held, along_x),
            }
        }
        panic!("no ratio settled; ended on {held}");
    }

    #[test]
    fn a_ratio_settles_across_many_decades() {
        // A falling measure in a power law, whose solution is 1e6.
        let (ratio, _) = solve_ratio(1.0, |r| Ok(1e12 / r));
        assert!((ratio / 1e6 - 1.0).abs() < 1e-5, "{ratio}");
        let (ratio, _) = solve_ratio(1.0, |r| Ok(3.0 / (1.0 + r * r)));
        assert!(
            (ratio * (1.0 + ratio * ratio) - 3.0).abs() < 1e-4,
            "{ratio}"
        );
    }

    #[test]
    fn a_ratio_that_cannot_be_laid_out_says_which_way_to_go() {
        let measure = |r: f64| {
            if r > 8.0 {
                Err(true)
            } else if r < 0.5 {
                Err(false)
            } else {
                Ok(4.0 / r)
            }
        };
        assert!((solve_ratio(1000.0, measure).0 - 2.0).abs() < 1e-4);
        assert!((solve_ratio(0.001, measure).0 - 2.0).abs() < 1e-4);
    }

    #[test]
    fn a_length_is_a_share_of_the_magnitude_of_the_frame() {
        let frame = ArrowFrame {
            length: 30.0,
            magnitude: 1e-307,
            scales: SCREEN_SCALES,
            key_length: 0.0,
        };
        assert_eq!(frame.length_of(1e-307), 30.0);
        assert_eq!(frame.length_of(5e-308), 15.0);
        // An arrow of no length stays so, whatever it is asked for.
        let empty = ArrowFrame {
            length: 0.0,
            ..frame
        };
        assert_eq!(empty.length_of(1e300), 0.0);
    }

    #[test]
    fn only_a_fixed_scale_on_the_screen_needs_no_layout_first() {
        let fixed = ArrowLength::PtPerUnit(3.0);
        assert_eq!(ArrowFrame::of(fixed, Direction::Screen).length, 3.0);
        assert_eq!(ArrowFrame::of(fixed, Direction::Data).length, 0.0);
        assert_eq!(
            ArrowFrame::of(ArrowLength::Auto, Direction::Screen).length,
            0.0
        );
    }

    #[test]
    fn a_head_keeps_its_size_from_the_reference_length_on() {
        for length in [15.0, 30.0, 300.0] {
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
    fn a_shorter_arrow_shrinks_whole_and_keeps_its_length() {
        for length in [10.0, 5.0, 1.0] {
            let outline = Outline::new(length, [1.0, 0.0], Pivot::Tail);
            let corners = outline.corners();
            assert_eq!(corners.len(), 7);
            assert_eq!(corners[3], [length, 0.0]);
            let head = corners[3][0] - corners[2][0];
            assert!(head <= length / 3.0 + 1e-12, "{length}: {head}");
            assert!(head > 0.0);
            // The head is wider than the shaft.
            assert!(corners[2][1] > corners[0][1], "{length}");
            assert_eq!(corners[2][1], -corners[4][1]);
        }
        // Above the floor, everything is in proportion: 10 pt is two thirds of the full size.
        let outline = Outline::new(10.0, [1.0, 0.0], Pivot::Tail);
        let corners = outline.corners();
        let third = 1.0 / 3.0;
        assert!((corners[2][0] - (10.0 - 5.0 * 2.0 * third)).abs() < 1e-12);
        assert!((corners[2][1] - 4.0 / 2.0 * 2.0 * third).abs() < 1e-12);
        assert!((corners[0][1] - 0.5 * 2.0 * third).abs() < 1e-12);
    }

    #[test]
    fn the_tiniest_arrow_keeps_a_shaft_of_a_pixel_and_a_head_twice_as_wide() {
        let outline = Outline::new(1.0, [1.0, 0.0], Pivot::Tail);
        let corners = outline.corners();
        assert_eq!(corners[0][1], MIN_SHAFT_WIDTH / 2.0);
        assert_eq!(corners[2][1], MIN_SHAFT_WIDTH);
        // Nothing of an arrow of no length sticks out along its direction.
        let none = Outline::new(0.0, [1.0, 0.0], Pivot::Tail);
        assert!(none.corners().iter().all(|corner| corner[0] == 0.0));
    }

    #[test]
    fn a_head_alone_is_centred_on_its_point() {
        let head = Outline::head([0.0, 1.0]);
        assert_eq!(head.corners(), [[-2.0, -2.5], [0.0, 2.5], [2.0, -2.5]]);
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
        let reach = Outline::new(20.0, [1.0, 0.0], Pivot::Middle).reach();
        assert_eq!(
            reach,
            Reach {
                left: 10.0,
                right: 10.0,
                down: 2.0,
                up: 2.0
            }
        );
    }
}
