//! The horizontal extent of the plot area that keeps every x tick label within given bounds.
//!
//! An x tick label is centered on its tick, so its ends are affine in the plot's left edge `L`
//! and width `D = R - L`: `L + pos × D ± half`. Every bound is then a constraint on `L` that is
//! affine in `D`:
//!
//! ```text
//! lower(D) = max(left, right_min - D, lo + half_i - pos_i × D)   ≤ L
//! upper(D) = min(left_max, right - D, hi - half_j - pos_j × D)   ≥ L
//! ```
//!
//! Each pair of a lower and an upper constraint bounds `D` on its own, and the plot must be at
//! least `min_width` wide, so the widest plot that fits is the smallest of those bounds, found
//! without iterating, provided it is at least `min_width` and every pair holds there; its left
//! edge is `lower(D)`. The widest plot is also the least inset: no other fit has a smaller left edge or
//! a larger right edge.

/// The largest distance, in points, by which a label may miss its bound and still count as
/// contained: far below anything visible, and far above the rounding of the solve.
pub(super) const CONTAINMENT_TOLERANCE: f64 = 1e-6;

/// The rounding, in points, allowed when deciding whether a pair of constraints is compatible
/// and whether an edge moved at all.
const ROUNDING_TOLERANCE: f64 = 1e-9;

/// The bisection steps that find the widest label that fits; see [`widest_label_that_fits`].
/// Each halves the interval, so 64 steps reach the resolution of `f64` for any label width.
const BISECTION_STEPS: usize = 64;

/// One x tick label as the inset sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TickSpan {
    /// Its tick's position along the axis, in `[0, 1]` (0 = the plot's left edge).
    pub pos: f64,
    /// Half its width.
    pub half: f64,
}

impl TickSpan {
    /// The span in the plot mirrored at x = 0.
    fn mirrored(self) -> TickSpan {
        TickSpan {
            pos: 1.0 - self.pos,
            half: self.half,
        }
    }

    /// The left and right ends of the label on the plot `left..right`.
    fn ends(self, left: f64, right: f64) -> (f64, f64) {
        let center = left + self.pos * (right - left);
        (center - self.half, center + self.half)
    }
}

/// Where the plot edges may go and where the labels must stay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Room {
    /// The left edge before any inset.
    pub left: f64,
    /// The right edge before any inset.
    pub right: f64,
    /// The furthest right the left edge may move; at least `left`.
    pub left_max: f64,
    /// The furthest left the right edge may move; at most `right`.
    pub right_min: f64,
    /// The smallest x a label may reach.
    pub lo: f64,
    /// The largest x a label may reach.
    pub hi: f64,
    /// The narrowest plot allowed.
    pub min_width: f64,
}

/// The plot edges [`fit_x_labels`] settles on, or that no edges keep every label in bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum XInset {
    /// Every label lies within the bounds.
    Contained {
        /// The left edge.
        left: f64,
        /// The right edge.
        right: f64,
    },
    /// No edges within the caps keep every label within the bounds.
    Overflowing,
}

/// The left and right edges of the plot area, inset from those of `room` just enough that
/// every label of `labels` lies in `room.lo..=room.hi`, within the caps `room.left_max` and
/// `room.right_min`, with the plot at least `room.min_width` wide.
///
/// When no edges within the caps can do that, the result is [`XInset::Overflowing`]: either the
/// labels need more inset than the caps allow, or a label at position 0 is wider than the room
/// between the left edge and `hi` (or one at position 1 than the room between `lo` and the right
/// edge), which no inset changes, or the room is narrower than `room.min_width` even without
/// labels.
///
/// When no label reaches past `lo` or `hi`, the edges are those of `room`, unchanged to the bit.
pub(super) fn fit_x_labels(room: &Room, labels: &[TickSpan]) -> XInset {
    let Some(width) = widest_plot(room, labels) else {
        return XInset::Overflowing;
    };
    let (left, right) = edges_for_width(room, labels, width);
    let contained = labels.iter().all(|l| {
        let (lo, hi) = l.ends(left, right);
        lo >= room.lo - CONTAINMENT_TOLERANCE && hi <= room.hi + CONTAINMENT_TOLERANCE
    });
    if contained {
        XInset::Contained { left, right }
    } else {
        XInset::Overflowing
    }
}

/// The largest width `w` such that the labels of `labels`, each cut to at most `w` wide (and
/// kept centered on its tick), all fit `room`: [`fit_x_labels`] gives [`XInset::Contained`] for
/// them. Labels no wider than the result therefore always fit. It is 0 when not even labels of
/// no width fit, and at most the widest label's width.
///
/// Cutting labels only relaxes the bounds, so whether they fit is monotonic in `w`, and
/// bisection finds the result.
pub(super) fn widest_label_that_fits(room: &Room, labels: &[TickSpan]) -> f64 {
    let fits = |half_max: f64| {
        let cut: Vec<TickSpan> = labels
            .iter()
            .map(|l| TickSpan {
                pos: l.pos,
                half: l.half.min(half_max),
            })
            .collect();
        matches!(fit_x_labels(room, &cut), XInset::Contained { .. })
    };
    let widest = labels.iter().map(|l| l.half).fold(0.0, f64::max);
    if fits(widest) {
        return 2.0 * widest;
    }
    if !fits(0.0) {
        return 0.0;
    }
    // Invariant: half widths up to `lo` fit, up to `hi` do not.
    let (mut lo, mut hi) = (0.0, widest);
    for _ in 0..BISECTION_STEPS {
        let mid = lo + (hi - lo) / 2.0;
        if mid <= lo || mid >= hi {
            break;
        }
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    2.0 * lo
}

/// A constraint `L >= a - b × D` (a lower one) or `L <= a - b × D` (an upper one) on the left
/// edge `L` of a plot `D` wide.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bound {
    a: f64,
    b: f64,
}

impl Bound {
    fn at(self, width: f64) -> f64 {
        self.a - self.b * width
    }
}

/// The lower and upper bounds on the left edge: the room's edges and caps, and both ends of
/// every label.
fn bounds(room: &Room, labels: &[TickSpan]) -> (Vec<Bound>, Vec<Bound>) {
    let mut lower = vec![
        Bound {
            a: room.left,
            b: 0.0,
        },
        Bound {
            a: room.right_min,
            b: 1.0,
        },
    ];
    let mut upper = vec![
        Bound {
            a: room.left_max,
            b: 0.0,
        },
        Bound {
            a: room.right,
            b: 1.0,
        },
    ];
    for l in labels {
        lower.push(Bound {
            a: room.lo + l.half,
            b: l.pos,
        });
        upper.push(Bound {
            a: room.hi - l.half,
            b: l.pos,
        });
    }
    (lower, upper)
}

/// The width of the widest plot, at least `room.min_width` wide, that keeps every bound, or
/// `None` when no width does.
///
/// A lower bound `a - b × D` and an upper bound `c - e × D` hold together when
/// `(e - b) × D <= c - a`: an upper limit on `D` when `e > b`, a lower one when `e < b`, and a
/// condition on the constants alone when `e == b`, so equal positions never divide.
fn widest_plot(room: &Room, labels: &[TickSpan]) -> Option<f64> {
    let (lower, upper) = bounds(room, labels);
    let (mut widest, mut narrowest) = (f64::INFINITY, room.min_width);
    for low in &lower {
        for up in &upper {
            let (slope, gap) = (up.b - low.b, up.a - low.a);
            if slope > 0.0 {
                widest = widest.min(gap / slope);
            } else if slope < 0.0 {
                narrowest = narrowest.max(gap / slope);
            } else if gap < -ROUNDING_TOLERANCE {
                return None;
            }
        }
    }
    // The pair of the room's left edge and right edge always bounds the width from above.
    (narrowest <= widest + ROUNDING_TOLERANCE).then_some(widest)
}

/// The left and right edges of the plot `width` wide whose left edge is the smallest the bounds
/// allow.
///
/// Where only one edge moves, it is computed from the other, unmoved edge in closed form, which
/// is exact for that side; layouts with one binding side therefore match the one-sided solve
/// to the bit.
fn edges_for_width(room: &Room, labels: &[TickSpan], width: f64) -> (f64, f64) {
    let (lower, _) = bounds(room, labels);
    let left = lower
        .iter()
        .map(|b| b.at(width))
        .fold(f64::NEG_INFINITY, f64::max);
    let right = left + width;
    let left_moved = left > room.left + ROUNDING_TOLERANCE;
    let right_moved = right < room.right - ROUNDING_TOLERANCE;
    match (left_moved, right_moved) {
        (false, false) => (room.left, room.right),
        (false, true) => (room.left, right_of(room, labels, room.left)),
        (true, false) => (left_of(room, labels, room.right), room.right),
        (true, true) => (left, right),
    }
}

/// The largest right edge, between `room.right_min` and `room.right`, that keeps the right end
/// of every label at or left of `room.hi` on a plot whose left edge is `left`.
fn right_of(room: &Room, labels: &[TickSpan], left: f64) -> f64 {
    right_edge(left, room.right, room.right_min, room.hi, labels)
}

/// The smallest left edge, between `room.left` and `room.left_max`, that keeps the left end of
/// every label at or right of `room.lo` on a plot whose right edge is `right`: the right edge
/// of the plot mirrored at x = 0, negated.
fn left_of(room: &Room, labels: &[TickSpan], right: f64) -> f64 {
    let mirrored: Vec<TickSpan> = labels.iter().map(|l| l.mirrored()).collect();
    -right_edge(-right, -room.left, -room.left_max, -room.lo, &mirrored)
}

/// The largest right edge in `min..=max` that keeps the right end of every label at or left of
/// `hi` on a plot whose left edge is `left`. A label at position 0 does not move with the right
/// edge and is left out.
fn right_edge(left: f64, max: f64, min: f64, hi: f64, labels: &[TickSpan]) -> f64 {
    labels
        .iter()
        .filter(|l| l.pos > 0.0)
        // The label's right end, (1 - pos) × left + pos × right + half, at `hi`.
        .map(|l| (hi - l.half - (1.0 - l.pos) * left) / l.pos)
        .fold(max, f64::min)
        .max(min)
}

#[cfg(test)]
mod tests {
    use super::super::{MIN_PLOT, fit_span};
    use super::*;
    use proptest::prelude::*;

    const ROOM: Room = Room {
        left: 30.0,
        right: 410.0,
        left_max: 114.0,
        right_min: 318.0,
        lo: 12.0,
        hi: 420.0,
        min_width: MIN_PLOT,
    };

    fn span(pos: f64, half: f64) -> TickSpan {
        TickSpan { pos, half }
    }

    /// The edges of a contained fit; panics on an overflowing one.
    fn contained(room: &Room, labels: &[TickSpan]) -> (f64, f64) {
        match fit_x_labels(room, labels) {
            XInset::Contained { left, right } => (left, right),
            other => panic!("{other:?}"),
        }
    }

    /// The left and right ends of each label on the plot `left..right`.
    fn ends(left: f64, right: f64, labels: &[TickSpan]) -> Vec<(f64, f64)> {
        labels.iter().map(|l| l.ends(left, right)).collect()
    }

    /// Asserts that every label lies within the bounds of `room` and that the extreme ends
    /// touch the bounds named by `touches` (left, right).
    fn assert_fits(room: &Room, labels: &[TickSpan], touches: (bool, bool)) -> (f64, f64) {
        let (left, right) = contained(room, labels);
        let ends = ends(left, right, labels);
        let lowest = ends.iter().map(|e| e.0).fold(f64::INFINITY, f64::min);
        let highest = ends.iter().map(|e| e.1).fold(f64::NEG_INFINITY, f64::max);
        assert!(
            lowest >= room.lo - 1e-9 && highest <= room.hi + 1e-9,
            "{ends:?}"
        );
        assert_eq!((lowest - room.lo).abs() < 1e-9, touches.0, "{ends:?}");
        assert_eq!((highest - room.hi).abs() < 1e-9, touches.1, "{ends:?}");
        (left, right)
    }

    #[test]
    fn labels_inside_leave_the_edges_alone() {
        let labels = [span(0.0, 10.0), span(0.5, 100.0), span(1.0, 0.0)];
        assert_eq!(contained(&ROOM, &labels), (ROOM.left, ROOM.right));
        assert_eq!(contained(&ROOM, &[]), (ROOM.left, ROOM.right));
    }

    #[test]
    fn the_last_label_moves_the_right_edge_by_its_overflow() {
        let (left, right) = contained(&ROOM, &[span(0.0, 5.0), span(1.0, 25.0)]);
        assert_eq!(left, ROOM.left);
        assert_eq!(right, ROOM.hi - 25.0);
    }

    #[test]
    fn the_first_label_moves_the_left_edge_by_its_overflow() {
        let (left, right) = contained(&ROOM, &[span(0.0, 25.0), span(1.0, 5.0)]);
        assert_eq!(left, ROOM.lo + 25.0);
        assert_eq!(right, ROOM.right);
    }

    #[test]
    fn inner_labels_on_both_sides_end_on_the_bounds() {
        // Category-like positions: both end labels overflow, and each edge moves the other's
        // label.
        let labels = [span(0.125, 80.0), span(0.375, 5.0), span(0.875, 90.0)];
        let (left, right) = assert_fits(&ROOM, &labels, (true, true));
        assert!(left > ROOM.left && right < ROOM.right);
    }

    #[test]
    fn close_interior_labels_end_exactly_on_the_bounds() {
        // Two wide labels near the middle: each edge moves the other's label almost as far as
        // itself, so the edges settle only where both bounds hold at once. The solution, with
        // left + 0.485 (right - left) - half = lo and left + 0.515 (right - left) + half = hi,
        // is left = 101, right = 331, within the caps.
        let room = Room {
            left: 33.3,
            right: 420.0,
            left_max: 114.0,
            right_min: 318.0,
            lo: 6.0,
            hi: 426.0,
            min_width: MIN_PLOT,
        };
        let labels = [span(0.485, 206.55), span(0.515, 206.55)];
        let (left, right) = assert_fits(&room, &labels, (true, true));
        assert!((left - 101.0).abs() < 1e-9 && (right - 331.0).abs() < 1e-9);
        // Closer still: the pieces rise almost as fast as the edges. Here 0.0002 (right - left)
        // = 420 - 2 half = 0.06, so the plot is 300 wide: left = 66, right = 366.
        let labels = [span(0.4999, 209.97), span(0.5001, 209.97)];
        let (left, right) = assert_fits(&room, &labels, (true, true));
        assert!((left - 66.0).abs() < 1e-6 && (right - 366.0).abs() < 1e-6);
    }

    #[test]
    fn the_widest_overflow_decides_not_the_last_label() {
        // The second to last label reaches further right than the last one.
        let labels = [span(0.5, 5.0), span(0.8, 150.0), span(1.0, 5.0)];
        let (left, _) = assert_fits(&ROOM, &labels, (false, true));
        assert_eq!(left, ROOM.left);
        let (left, right) = contained(&ROOM, &labels);
        assert!(ends(left, right, &labels)[2].1 < ROOM.hi);
    }

    #[test]
    fn labels_that_do_not_fit_overflow() {
        // Wider than the room the caps leave.
        let labels = [span(0.0, 500.0), span(0.5, 1000.0), span(1.0, 500.0)];
        assert_eq!(fit_x_labels(&ROOM, &labels), XInset::Overflowing);
        // A label at position 0 does not move with the right edge: when the room between the
        // left edge and `hi` is narrower than the label, no inset helps.
        let narrow = Room { hi: 40.0, ..ROOM };
        assert_eq!(
            fit_x_labels(&narrow, &[span(0.0, 15.0)]),
            XInset::Overflowing
        );
    }

    #[test]
    fn the_plot_keeps_its_minimum_width() {
        // A room whose right edge lies left of its left edge, as when wide y tick labels push
        // the left edge past the right one: no plot fits, with or without labels.
        let crossed = Room {
            left: 528.0,
            right: 132.0,
            left_max: 528.0,
            right_min: 132.0,
            lo: 6.0,
            hi: 138.0,
            min_width: MIN_PLOT,
        };
        assert_eq!(fit_x_labels(&crossed, &[]), XInset::Overflowing);
        assert_eq!(
            fit_x_labels(&crossed, &[span(1.0, 7.5)]),
            XInset::Overflowing
        );
        assert_eq!(widest_label_that_fits(&crossed, &[span(1.0, 7.5)]), 0.0);
        // A room just as wide as the minimum fits a plot of exactly that width; a narrower one
        // does not.
        let tight = Room {
            left: 100.0,
            right: 100.0 + MIN_PLOT,
            left_max: 100.0,
            right_min: 100.0,
            lo: 6.0,
            hi: 138.0,
            min_width: MIN_PLOT,
        };
        assert_eq!(
            contained(&tight, &[span(1.0, 5.0)]),
            (100.0, 100.0 + MIN_PLOT)
        );
        let narrower = Room {
            right: 100.0 + 0.5 * MIN_PLOT,
            ..tight
        };
        assert_eq!(fit_x_labels(&narrower, &[]), XInset::Overflowing);
        // A label that would need the plot narrower than the minimum overflows: its tick at
        // the right edge may move in to `left + MIN_PLOT` and no further.
        let room = Room {
            right_min: 0.0,
            left_max: 100.0,
            right: 130.0,
            ..tight
        };
        let at_minimum = 2.0 * (room.hi - (room.left + MIN_PLOT));
        assert_eq!(
            contained(&room, &[span(1.0, at_minimum / 2.0)]),
            (100.0, 100.0 + MIN_PLOT)
        );
        assert_eq!(
            fit_x_labels(&room, &[span(1.0, at_minimum / 2.0 + 1e-3)]),
            XInset::Overflowing
        );
        let widest = widest_label_that_fits(&room, &[span(1.0, 100.0)]);
        assert!((widest - at_minimum).abs() < 1e-6, "{widest}");
    }

    /// The labels of `labels` cut to at most `width` wide.
    fn cut(labels: &[TickSpan], width: f64) -> Vec<TickSpan> {
        labels
            .iter()
            .map(|l| span(l.pos, l.half.min(width / 2.0)))
            .collect()
    }

    fn fits(room: &Room, labels: &[TickSpan]) -> bool {
        matches!(fit_x_labels(room, labels), XInset::Contained { .. })
    }

    #[test]
    fn the_widest_label_that_fits_is_the_largest_cut_that_fits() {
        // A single label at the right end: it may reach from its tick at the capped right edge
        // to `hi`, so it fits up to 2 × (hi - right_min) = 204 wide.
        let widest = widest_label_that_fits(&ROOM, &[span(1.0, 500.0)]);
        assert!(
            (widest - 2.0 * (ROOM.hi - ROOM.right_min)).abs() < 1e-6,
            "{widest}"
        );
        // Labels that fit give their own widest width.
        let labels = [span(0.0, 10.0), span(1.0, 20.0)];
        assert_eq!(widest_label_that_fits(&ROOM, &labels), 40.0);
        // A label at position 0 that no inset helps: it may reach from the room's left edge
        // to `hi`, so it fits up to 2 × (40 - 30) = 20 wide.
        let narrow = Room { hi: 40.0, ..ROOM };
        let widest = widest_label_that_fits(&narrow, &[span(0.0, 15.0)]);
        assert!((widest - 20.0).abs() < 1e-6, "{widest}");
        // Labels that cannot fit at any width give 0: a tick at the left edge can never reach
        // `lo` past the left cap.
        let closed = Room { lo: 200.0, ..ROOM };
        assert_eq!(widest_label_that_fits(&closed, &[span(0.0, 15.0)]), 0.0);
        // Several labels that do not fit: the result fits, a little more does not.
        let labels = [span(0.0, 500.0), span(0.5, 1000.0), span(1.0, 500.0)];
        let widest = widest_label_that_fits(&ROOM, &labels);
        assert!(widest > 0.0 && widest < 1000.0, "{widest}");
        assert!(fits(&ROOM, &cut(&labels, widest)));
        assert!(!fits(&ROOM, &cut(&labels, widest + 1e-6)));
    }

    #[test]
    fn a_label_as_wide_as_the_room_keeps_the_widest_plot() {
        // The label fills lo..hi exactly, so its center is fixed at 57 and only the widest
        // plot with that center is wanted: the left edge stays at 12.
        let room = Room {
            left: 12.0,
            right: 102.0,
            left_max: 34.5,
            right_min: 79.5,
            lo: 6.0,
            hi: 108.0,
            min_width: MIN_PLOT,
        };
        for pos in [0.5001, 0.515] {
            let (left, right) = contained(&room, &[span(pos, 51.0)]);
            assert_eq!(left, 12.0, "{pos}");
            let want = 12.0 + 45.0 / pos;
            assert!((right - want).abs() < 1e-9, "{pos}: {right} vs {want}");
        }
    }

    /// Exact references for the property test, on the exact values of the `f64` inputs.
    mod exact {
        use super::super::{Room, TickSpan};
        use crate::exact::Dyadic;
        use std::cmp::Ordering;

        fn d(v: f64) -> Dyadic {
            Dyadic::from_f64(v)
        }

        fn sign(v: &Dyadic) -> Ordering {
            let f = v.div_to_f64(&d(1.0)).unwrap_or(0.0);
            f.partial_cmp(&0.0).unwrap_or(Ordering::Equal)
        }

        /// `L >= a - b D` or `L <= a - b D`, exactly.
        struct Bound {
            a: Dyadic,
            b: Dyadic,
        }

        fn bounds(room: &Room, labels: &[TickSpan]) -> (Vec<Bound>, Vec<Bound>) {
            let bound = |a: Dyadic, b: f64| Bound { a, b: d(b) };
            let mut lower = vec![bound(d(room.left), 0.0), bound(d(room.right_min), 1.0)];
            let mut upper = vec![bound(d(room.left_max), 0.0), bound(d(room.right), 1.0)];
            for l in labels {
                lower.push(bound(d(room.lo).add(&d(l.half)), l.pos));
                upper.push(bound(d(room.hi).sub(&d(l.half)), l.pos));
            }
            (lower, upper)
        }

        /// The width `num / den` (with `den > 0`) of the widest plot, at least `min_width` wide,
        /// that keeps every bound and the smallest left edge for it, both rounded to `f64`;
        /// `None` when no width does.
        pub fn widest(room: &Room, labels: &[TickSpan]) -> Option<(f64, f64)> {
            let (lower, upper) = bounds(room, labels);
            // The smallest of the upper limits gap / slope over pairs with slope > 0.
            let mut best: Option<(Dyadic, Dyadic)> = None;
            for low in &lower {
                for up in &upper {
                    let slope = up.b.sub(&low.b);
                    if sign(&slope) != Ordering::Greater {
                        continue;
                    }
                    let gap = up.a.sub(&low.a);
                    let smaller = best.as_ref().is_none_or(|(n, m)| {
                        // gap / slope < n / m
                        sign(&n.mul(&slope).sub(&gap.mul(m))) == Ordering::Greater
                    });
                    if smaller {
                        best = Some((gap, slope));
                    }
                }
            }
            let (num, den) = best?;
            // The plot is at least `min_width` wide: num >= min_width den.
            if sign(&num.sub(&d(room.min_width).mul(&den))) == Ordering::Less {
                return None;
            }
            // Every pair must hold at that width: (e - b) num <= (c - a) den.
            for low in &lower {
                for up in &upper {
                    let lhs = up.b.sub(&low.b).mul(&num);
                    let rhs = up.a.sub(&low.a).mul(&den);
                    if sign(&rhs.sub(&lhs)) == Ordering::Less {
                        return None;
                    }
                }
            }
            // The smallest left edge: the largest lower bound, (a den - b num) / den.
            let mut left: Option<Dyadic> = None;
            for low in &lower {
                let v = low.a.mul(&den).sub(&low.b.mul(&num));
                if left
                    .as_ref()
                    .is_none_or(|l| sign(&v.sub(l)) == Ordering::Greater)
                {
                    left = Some(v);
                }
            }
            let left = left?.div_to_f64(&den)?;
            Some((num.div_to_f64(&den)?, left))
        }
    }

    /// A tick position: often an end, often close to another tick.
    fn position() -> impl Strategy<Value = f64> {
        prop_oneof![
            2 => Just(0.0),
            2 => Just(1.0),
            1 => Just(1e-16),
            1 => Just(1.0 - 1e-16),
            6 => 0.0..=1.0_f64,
        ]
    }

    /// Half a label's width: mostly a fraction of the plot, sometimes most of it.
    fn half(width: f64) -> impl Strategy<Value = f64> + Clone {
        prop_oneof![3 => 0.0..=(0.15 * width), 1 => 0.0..=(0.6 * width)]
    }

    /// Up to four labels; one more may sit at or next to the first.
    fn labels(width: f64) -> impl Strategy<Value = Vec<TickSpan>> {
        (
            prop::collection::vec((position(), half(width)), 0..=4),
            prop_oneof![Just(None), (0usize..6, half(width)).prop_map(Some)],
        )
            .prop_map(|(spans, near)| {
                let mut spans: Vec<TickSpan> = spans
                    .into_iter()
                    .map(|(pos, half)| TickSpan { pos, half })
                    .collect();
                if let (Some((k, half)), Some(first)) = (near, spans.first().copied()) {
                    let delta = [0.0, 1e-16, 1e-12, 1e-9, 1e-6, 1e-3][k];
                    let pos = if first.pos + delta <= 1.0 {
                        first.pos + delta
                    } else {
                        first.pos - delta
                    };
                    spans.push(TickSpan { pos, half });
                }
                spans
            })
    }

    /// The width of a room: mostly roomy, sometimes about [`MIN_PLOT`] or less, down to a right
    /// edge left of the left one, as when wide y tick labels push the left edge past it.
    fn room_width() -> impl Strategy<Value = f64> {
        prop_oneof![
            6 => 1.0..500.0_f64,
            1 => Just(MIN_PLOT),
            1 => (0.0..=2.0_f64).prop_map(|k| k * MIN_PLOT),
            1 => -400.0..0.0_f64,
        ]
    }

    fn room_and_labels() -> impl Strategy<Value = (Room, Vec<TickSpan>)> {
        (
            0.0..60.0_f64,
            room_width(),
            0.0..=0.5_f64,
            0.0..=0.5_f64,
            0.0..20.0_f64,
            0.0..20.0_f64,
        )
            .prop_flat_map(|(left, width, cap_left, cap_right, pad_lo, pad_hi)| {
                let right = left + width;
                // Caps and labels scale with the room's extent, whichever way it points.
                let extent = width.abs().max(MIN_PLOT);
                let room = Room {
                    left,
                    right,
                    left_max: left + cap_left * extent,
                    right_min: right - cap_right * extent,
                    lo: left - pad_lo,
                    hi: left.max(right) + pad_hi,
                    min_width: MIN_PLOT,
                };
                labels(extent).prop_map(move |labels| (room, labels))
            })
    }

    /// Asserts what layout relies on for a contained fit `left..right` of `labels` in `room`:
    /// the plot is at least `room.min_width` wide, and once [`fit_span`] has kept it on a canvas
    /// that ends at the room's right edge or `hi`, every label still lies within the bounds.
    fn assert_placeable(
        room: &Room,
        labels: &[TickSpan],
        left: f64,
        right: f64,
    ) -> Result<(), TestCaseError> {
        prop_assert!(
            right - left >= room.min_width - ROUNDING_TOLERANCE,
            "{left} {right}"
        );
        let (x, width) = fit_span(left, right, room.right.max(room.hi));
        for l in labels {
            let (lo, hi) = l.ends(x, x + width);
            prop_assert!(
                lo >= room.lo - CONTAINMENT_TOLERANCE && hi <= room.hi + CONTAINMENT_TOLERANCE,
                "{l:?} on {x}..{}: {lo}..{hi}",
                x + width
            );
        }
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20_000))]

        #[test]
        fn the_fit_is_the_widest_plot_the_exact_bounds_allow(
            (room, labels) in room_and_labels()
        ) {
            match (fit_x_labels(&room, &labels), exact::widest(&room, &labels)) {
                (XInset::Contained { left, right }, Some((width, least_left))) => {
                    prop_assert!(((right - left) - width).abs() <= 1e-6, "{left} {right} {width}");
                    prop_assert!((left - least_left).abs() <= 1e-6, "{left} vs {least_left}");
                    assert_placeable(&room, &labels, left, right)?;
                }
                (XInset::Overflowing, Some(want)) => {
                    prop_assert!(false, "overflowing, but {want:?} fits");
                }
                // Not exactly feasible: contained only within the tolerance.
                (XInset::Contained { left, right }, None) => {
                    for l in &labels {
                        let (lo, hi) = l.ends(left, right);
                        prop_assert!(lo >= room.lo - 1e-6 && hi <= room.hi + 1e-6);
                    }
                    assert_placeable(&room, &labels, left, right)?;
                }
                // The labels cut to the widest width that fits do fit, unless not even labels
                // of no width fit, and no label that fits is cut.
                (XInset::Overflowing, None) => {
                    let widest = widest_label_that_fits(&room, &labels);
                    prop_assert!(
                        fits(&room, &cut(&labels, widest))
                            || (widest == 0.0 && !fits(&room, &cut(&labels, 0.0))),
                        "{widest}"
                    );
                    let widest_half = labels.iter().map(|l| l.half).fold(0.0, f64::max);
                    prop_assert!(widest == 0.0 || widest < 2.0 * widest_half, "{widest}");
                }
            }
        }
    }
}
