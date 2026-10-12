//! The horizontal extent of the plot area that keeps every x tick label within given bounds.
//!
//! An x tick label is centered on its tick, so it moves with the plot edges. Where a label would
//! reach past a bound, the plot edge on that side moves in by exactly the overflow. One
//! function handles the right edge; the left edge is the right edge of the plot mirrored at
//! x = 0, which negation does exactly.

/// The most rounds [`fit_x_labels`] takes. Each round settles the right edge for the current
/// left edge and then the left edge for that right edge. One round settles labels that reach
/// past one side only; labels past both sides converge geometrically, and the caps stop
/// labels that do not fit at all.
const MAX_ROUNDS: usize = 16;

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

/// The left and right edges of the plot area, inset from those of `room` just enough that
/// every label of `labels` lies in `room.lo..=room.hi`, within the caps `room.left_max` and
/// `room.right_min`. Labels that need more than the caps allow reach past `lo` or `hi`; so does
/// a label at position 0 wider than the room right of the left edge, and its mirror image.
///
/// When no label reaches past `lo` or `hi`, the edges are those of `room`, unchanged to the bit.
pub(super) fn fit_x_labels(room: &Room, labels: &[TickSpan]) -> (f64, f64) {
    let mirrored: Vec<TickSpan> = labels.iter().map(|l| l.mirrored()).collect();
    let (mut left, mut right) = (room.left, room.right);
    for _ in 0..MAX_ROUNDS {
        let next_right = right_edge(left, room.right, room.right_min, room.hi, labels);
        let next_left = -right_edge(-next_right, -room.left, -room.left_max, -room.lo, &mirrored);
        let settled = next_left == left && next_right == right;
        (left, right) = (next_left, next_right);
        if settled {
            break;
        }
    }
    (left, right)
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

    /// The left and right ends of each label on the plot `left..right`.
    fn ends(left: f64, right: f64, labels: &[TickSpan]) -> Vec<(f64, f64)> {
        labels
            .iter()
            .map(|l| {
                let center = left + l.pos * (right - left);
                (center - l.half, center + l.half)
            })
            .collect()
    }

    #[test]
    fn labels_inside_leave_the_edges_alone() {
        let labels = [span(0.0, 10.0), span(0.5, 100.0), span(1.0, 0.0)];
        assert_eq!(fit_x_labels(&ROOM, &labels), (ROOM.left, ROOM.right));
        assert_eq!(fit_x_labels(&ROOM, &[]), (ROOM.left, ROOM.right));
    }

    #[test]
    fn the_last_label_moves_the_right_edge_by_its_overflow() {
        let (left, right) = fit_x_labels(&ROOM, &[span(0.0, 5.0), span(1.0, 25.0)]);
        assert_eq!(left, ROOM.left);
        assert_eq!(right, ROOM.hi - 25.0);
    }

    #[test]
    fn the_first_label_moves_the_left_edge_by_its_overflow() {
        let (left, right) = fit_x_labels(&ROOM, &[span(0.0, 25.0), span(1.0, 5.0)]);
        assert_eq!(left, ROOM.lo + 25.0);
        assert_eq!(right, ROOM.right);
    }

    #[test]
    fn inner_labels_on_both_sides_end_on_the_bounds() {
        // Category-like positions: both end labels overflow, and each edge moves the other's
        // label.
        let labels = [span(0.125, 80.0), span(0.375, 5.0), span(0.875, 90.0)];
        let (left, right) = fit_x_labels(&ROOM, &labels);
        assert!(left > ROOM.left && right < ROOM.right);
        let ends = ends(left, right, &labels);
        assert!((ends[0].0 - ROOM.lo).abs() < 1e-9, "{ends:?}");
        assert!((ends[2].1 - ROOM.hi).abs() < 1e-9, "{ends:?}");
        assert!(ends[1].0 >= ROOM.lo && ends[1].1 <= ROOM.hi);
    }

    #[test]
    fn the_widest_overflow_decides_not_the_last_label() {
        // The second to last label reaches further right than the last one.
        let labels = [span(0.5, 5.0), span(0.8, 150.0), span(1.0, 5.0)];
        let (left, right) = fit_x_labels(&ROOM, &labels);
        assert_eq!(left, ROOM.left);
        let ends = ends(left, right, &labels);
        assert!((ends[1].1 - ROOM.hi).abs() < 1e-9, "{ends:?}");
        assert!(ends[2].1 < ROOM.hi);
    }

    #[test]
    fn the_caps_stop_labels_that_do_not_fit() {
        let labels = [span(0.0, 500.0), span(0.5, 1000.0), span(1.0, 500.0)];
        assert_eq!(
            fit_x_labels(&ROOM, &labels),
            (ROOM.left_max, ROOM.right_min)
        );
        // A right edge already left of its cap does not move.
        let room = Room {
            right: 300.0,
            right_min: 300.0,
            ..ROOM
        };
        assert_eq!(fit_x_labels(&room, &labels).1, 300.0);
    }
}
