//! The horizontal extent of the plot area that keeps every x tick label within given bounds.
//!
//! An x tick label is centered on its tick, so it moves with the plot edges: its ends are affine
//! in the left and right edge. Where a label would reach past a bound, the plot edge on that side
//! moves in by exactly the overflow. One function bounds the right edge; the left edge is the
//! right edge of the plot mirrored at x = 0, which negation does exactly.
//!
//! The bounds couple the edges: moving the left edge in moves every label right, which can push
//! another label past the right bound, and the other way round. Writing `F(left)` for the largest
//! right edge the labels allow and `G(right)` for the smallest left edge, the edges sought are the
//! smallest fixed point of `left = G(F(left))`. `G ∘ F` is a nondecreasing convex piecewise-affine
//! function, each of whose pieces lies below it, so Newton steps on its pieces approach that fixed
//! point from below and reach it after at most one step per piece.

/// The largest distance, in points, by which a label may miss its bound and still count as
/// contained: far below anything visible, and far above the rounding of the solve.
const CONTAINMENT_TOLERANCE: f64 = 1e-6;

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
}

/// The plot edges [`fit_x_labels`] settles on, and whether they keep every label in bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum XInset {
    /// Every label lies within the bounds.
    Contained {
        /// The left edge.
        left: f64,
        /// The right edge.
        right: f64,
    },
    /// No edges within the caps keep every label within the bounds. The edges are the least
    /// inset the capped bounds settle on; some labels reach past the bounds.
    Overflowing {
        /// The left edge.
        left: f64,
        /// The right edge.
        right: f64,
    },
}

/// The left and right edges of the plot area, inset from those of `room` just enough that
/// every label of `labels` lies in `room.lo..=room.hi`, within the caps `room.left_max` and
/// `room.right_min`.
///
/// When no edges within the caps can do that, the result is [`XInset::Overflowing`]: either the
/// labels need more inset than the caps allow, or a label at position 0 is wider than the room
/// between the left edge and `hi` (or one at position 1 than the room between `lo` and the right
/// edge), which no inset changes. Those edges are the smallest fixed point of the bounds clamped
/// to the caps, so an edge whose labels need more stops at its cap.
///
/// When no label reaches past `lo` or `hi`, the edges are those of `room`, unchanged to the bit.
pub(super) fn fit_x_labels(room: &Room, labels: &[TickSpan]) -> XInset {
    let mirrored: Vec<TickSpan> = labels.iter().map(|l| l.mirrored()).collect();
    let right_of = |left: f64| right_edge(left, room.right, room.hi, labels);
    // The left edge is the right edge of the mirrored plot, negated. Negating both the other
    // edge and the result leaves the slope as it is.
    let left_of = |right: f64| {
        let piece = right_edge(-right, -room.left, -room.lo, &mirrored);
        Piece {
            value: -piece.value,
            slope: piece.slope,
        }
    };

    // Newton steps on `left = G(F(left))`, from `room.left`. Each step lands on the fixed point
    // of the piece in use, which is never past the smallest fixed point, and uses each piece at
    // most once; `G ∘ F` has at most one piece per bound and edge.
    let mut left = room.left;
    for _ in 0..2 * labels.len() + 2 {
        let right = right_of(left);
        let next = left_of(right.value);
        if next.value <= left {
            break;
        }
        let slope = next.slope * right.slope;
        if slope >= 1.0 {
            // The pieces rise at least as fast as the left edge: no fixed point below the cap.
            left = f64::INFINITY;
            break;
        }
        // Where the piece through (left, next.value) meets the diagonal.
        let fixed = (next.value - slope * left) / (1.0 - slope);
        if fixed <= left {
            break;
        }
        left = fixed;
    }
    // The caps: the left edge stops at its own cap, and at the left edge the labels need when the
    // right edge stops at its cap.
    let left = left.min(room.left_max).min(left_of(room.right_min).value);
    let right = right_of(left).value.max(room.right_min);

    let contained = labels.iter().all(|l| {
        let (lo, hi) = l.ends(left, right);
        lo >= room.lo - CONTAINMENT_TOLERANCE && hi <= room.hi + CONTAINMENT_TOLERANCE
    });
    if contained {
        XInset::Contained { left, right }
    } else {
        XInset::Overflowing { left, right }
    }
}

/// A value of an edge bound and its slope with respect to the other edge.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Piece {
    value: f64,
    slope: f64,
}

/// The largest right edge, at most `max`, that keeps the right end of every label at or left of
/// `hi` on a plot whose left edge is `left`, with the slope of the binding bound (0 for `max`).
/// Of bounds that bind equally, the one that falls fastest as `left` grows is reported. A label
/// at position 0 does not move with the right edge and is left out.
fn right_edge(left: f64, max: f64, hi: f64, labels: &[TickSpan]) -> Piece {
    let mut bound = Piece {
        value: max,
        slope: 0.0,
    };
    for l in labels.iter().filter(|l| l.pos > 0.0) {
        // The label's right end, (1 - pos) × left + pos × right + half, at `hi`.
        let value = (hi - l.half - (1.0 - l.pos) * left) / l.pos;
        let slope = -(1.0 - l.pos) / l.pos;
        if value < bound.value || (value == bound.value && slope < bound.slope) {
            bound = Piece { value, slope };
        }
    }
    bound
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOM: Room = Room {
        left: 30.0,
        right: 410.0,
        left_max: 114.0,
        right_min: 318.0,
        lo: 12.0,
        hi: 420.0,
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
    fn labels_that_do_not_fit_stop_at_the_caps_and_overflow() {
        // Wider than the room the caps leave: the edges stop at the caps.
        let labels = [span(0.0, 500.0), span(0.5, 1000.0), span(1.0, 500.0)];
        assert_eq!(
            fit_x_labels(&ROOM, &labels),
            XInset::Overflowing {
                left: ROOM.left_max,
                right: ROOM.right_min,
            }
        );
        // A right edge already left of its cap does not move.
        let room = Room {
            right: 300.0,
            right_min: 300.0,
            ..ROOM
        };
        assert!(matches!(
            fit_x_labels(&room, &labels),
            XInset::Overflowing { right: 300.0, .. }
        ));
        // A label at position 0 does not move with the right edge: when the room between the
        // left edge and `hi` is narrower than the label, no inset helps.
        let narrow = Room { hi: 40.0, ..ROOM };
        assert!(matches!(
            fit_x_labels(&narrow, &[span(0.0, 15.0)]),
            XInset::Overflowing { .. }
        ));
    }
}
