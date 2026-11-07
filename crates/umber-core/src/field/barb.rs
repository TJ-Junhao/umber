//! The shape of a wind barb: a staff from the sample, with flags and lines at its outer end.
//!
//! The speed is rounded to the nearest 5, halves upward, and written from the outer end of the
//! staff inwards: a flag for each 50, a full line for each 10, a half line for a remaining 5.
//!
//! Every size is a fixed share of the staff length `s`:
//!
//! - a full line rises [`LINE_RISE`] (`0.4 s`) to the side of the staff and leans
//!   [`LINE_LEAN`] (`0.125 s`) towards its outer end;
//! - a half line goes the same way for half the length: `0.2 s` and `0.0625 s`;
//! - a flag is a filled triangle with a base of [`FLAG_BASE`] (`0.25 s`) on the staff and its
//!   apex [`LINE_RISE`] to the side of the base's outer end;
//! - flags touch, and lines stand [`STEP`] (`0.125 s`) apart, from each other and from the
//!   last flag;
//! - a half line alone stands [`STEP`] in from the end of the staff, so that it does not read
//!   as a full line cut short;
//! - lines are stroked [`STROKE`] (`0.04 s`) wide, with round ends;
//! - a speed that rounds to 0 is an open circle of radius [`CALM_RADIUS`] (`0.125 s`) around
//!   the sample.
//!
//! Above 200, flags alone would pass the sample, so the staff grows inward by what they need
//! and one step more; the outer end moves out by the same length.

use super::{Reach, magnitude};
use crate::{SizeSpec, Theme};

/// How far a full line and a flag rise to the side of the staff, as a share of its length.
pub(crate) const LINE_RISE: f64 = 0.4;
/// How far a full line leans towards the outer end of the staff, as a share of its length.
pub(crate) const LINE_LEAN: f64 = 0.125;
/// The length of a flag's base along the staff, as a share of its length.
pub(crate) const FLAG_BASE: f64 = 0.25;
/// The distance between two lines, and from the last flag to the first line, as a share of the
/// length of the staff.
pub(crate) const STEP: f64 = 0.125;
/// The radius of the circle of a calm, as a share of the length of the staff.
pub(crate) const CALM_RADIUS: f64 = 0.125;
/// The width of the staff, the lines and the circle, as a share of the length of the staff.
pub(crate) const STROKE: f64 = 0.04;
/// The length of a staff in points at a font scale of 1, when no size is given.
pub(crate) const STAFF: f64 = 24.0;

/// The speed a flag stands for.
const FLAG_SPEED: u64 = 50;
/// The speed a full line stands for.
const FULL_SPEED: u64 = 10;
/// The step of the speeds a barb can show.
const SPEED_STEP: f64 = 5.0;
/// The most flags a barb draws: far beyond any wind, and it bounds the shape.
const MAX_FLAGS: u64 = 64;

/// The length, in points, of a staff that asks for `size` under `theme`: [`STAFF`] times the
/// font scale is the size that `size` resolves against.
pub(crate) fn staff_length(theme: &Theme, size: SizeSpec) -> f64 {
    let automatic = STAFF as f32 * theme.font_scale();
    f64::from(size.resolve(automatic))
}

/// What a barb shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Feathers {
    /// The flags, 50 each.
    pub flags: u64,
    /// The full lines, 10 each.
    pub full: u64,
    /// Whether a half line, 5, remains.
    pub half: bool,
}

impl Feathers {
    /// What a barb shows for `speed`, which is not NaN, rounded to the nearest 5, halves
    /// upward: 2.5 shows a half line, 7.5 a full line, 47.5 a flag. Dividing by 5 keeps these
    /// halves exact.
    pub(crate) fn of(speed: f64) -> Feathers {
        // A speed beyond every flag saturates, and `MAX_FLAGS` bounds it.
        let steps = (speed / SPEED_STEP + 0.5).floor() as u64;
        let rounded = steps.saturating_mul(SPEED_STEP as u64);
        Feathers {
            flags: (rounded / FLAG_SPEED).min(MAX_FLAGS),
            full: rounded % FLAG_SPEED / FULL_SPEED,
            half: !rounded.is_multiple_of(FULL_SPEED),
        }
    }

    /// Whether the barb shows no wind.
    pub(crate) fn is_calm(self) -> bool {
        self == Feathers {
            flags: 0,
            full: 0,
            half: false,
        }
    }
}

/// A segment of a barb, from one end to the other.
pub(crate) type Segment = [[f64; 2]; 2];
/// The corners of a flag.
pub(crate) type Flag = [[f64; 2]; 3];

/// A barb for one sample, in points relative to it, y up.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Barb {
    /// A speed that rounds to 0: a circle around the sample.
    Calm {
        /// The radius of the circle.
        radius: f64,
    },
    /// A staff with its lines and flags.
    Wind {
        /// The staff, then each line.
        lines: Vec<Segment>,
        /// The flags.
        flags: Vec<Flag>,
    },
}

impl Barb {
    /// The barb of the wind `(u, v)`, which blows towards `(u, v)`, on a staff `staff` pt long.
    /// The staff starts at the sample and runs against the wind. Lines and flags lie to the
    /// right when looking from the sample along the staff, or to the left with `flip`.
    pub(crate) fn new(u: f64, v: f64, staff: f64, flip: bool) -> Barb {
        let speed = magnitude(u, v);
        let feathers = Feathers::of(speed);
        if feathers.is_calm() {
            return Barb::Calm {
                radius: CALM_RADIUS * staff,
            };
        }
        // Along the staff, away from the sample, and to the side the lines lie on.
        let (dx, dy) = (-u / speed, -v / speed);
        let (sx, sy) = if flip { (-dy, dx) } else { (dy, -dx) };
        let at = |along: f64, side: f64| [along * dx + side * sx, along * dy + side * sy];
        let line_count = feathers.full + u64::from(feathers.half);
        let flags_end = feathers.flags as f64 * FLAG_BASE;
        let lines_start = match (feathers.flags, feathers.full) {
            (0, 0) => STEP,
            (0, _) => 0.0,
            _ => flags_end + STEP,
        };
        let inner = match line_count {
            0 => flags_end,
            n => lines_start + (n - 1) as f64 * STEP,
        };
        // Measured from the outer end of the staff, as a share of its nominal length.
        let length = staff * f64::max(1.0, inner + STEP);
        let from_end = |share: f64| length - share * staff;
        let mut lines = Vec::with_capacity(1 + line_count as usize);
        lines.push([at(0.0, 0.0), at(length, 0.0)]);
        for i in 0..feathers.full {
            let root = from_end(lines_start + i as f64 * STEP);
            let tip = at(root + LINE_LEAN * staff, LINE_RISE * staff);
            lines.push([at(root, 0.0), tip]);
        }
        if feathers.half {
            let root = from_end(lines_start + feathers.full as f64 * STEP);
            let tip = at(root + LINE_LEAN * staff / 2.0, LINE_RISE * staff / 2.0);
            lines.push([at(root, 0.0), tip]);
        }
        let flags = (0..feathers.flags)
            .map(|i| {
                let outer = from_end(i as f64 * FLAG_BASE);
                [
                    at(outer, 0.0),
                    at(outer, LINE_RISE * staff),
                    at(outer - FLAG_BASE * staff, 0.0),
                ]
            })
            .collect();
        Barb::Wind { lines, flags }
    }

    /// How far the barb reaches from its sample, strokes `width` pt wide included.
    pub(crate) fn reach(&self, width: f64) -> Reach {
        match self {
            Barb::Calm { radius } => Reach::around(radius + width / 2.0),
            Barb::Wind { lines, flags } => lines
                .iter()
                .flatten()
                .chain(flags.iter().flatten())
                .fold(Reach::default(), |reach, point| reach.holding(*point))
                .widened(width / 2.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feathers(speed: f64) -> (u64, u64, bool) {
        let shown = Feathers::of(speed);
        (shown.flags, shown.full, shown.half)
    }

    #[test]
    fn each_speed_is_written_in_flags_and_lines() {
        let expected = [
            (0, 0, false),
            (0, 0, true),
            (0, 1, false),
            (0, 1, true),
            (0, 2, false),
            (0, 2, true),
            (0, 3, false),
            (0, 3, true),
            (0, 4, false),
            (0, 4, true),
            (1, 0, false),
            (1, 0, true),
            (1, 1, false),
            (1, 1, true),
        ];
        for (i, shown) in expected.iter().enumerate() {
            assert_eq!(feathers(5.0 * i as f64), *shown, "speed {}", 5 * i);
        }
    }

    #[test]
    fn halves_round_up() {
        assert_eq!(feathers(2.5), (0, 0, true));
        assert_eq!(feathers(2.499_999), (0, 0, false));
        assert_eq!(feathers(7.5), (0, 1, false));
        assert_eq!(feathers(7.499_999), (0, 0, true));
        assert_eq!(feathers(47.5), (1, 0, false));
        assert_eq!(feathers(47.499_999), (0, 4, true));
    }

    #[test]
    fn calm_is_a_circle_around_the_sample() {
        assert_eq!(Barb::new(0.0, 0.0, 24.0, false), Barb::Calm { radius: 3.0 });
        assert_eq!(Barb::new(1.0, 1.0, 24.0, false), Barb::Calm { radius: 3.0 });
        assert_eq!(
            Barb::new(0.0, 0.0, 24.0, false).reach(1.0),
            Reach::around(3.5)
        );
    }

    /// The lines of the barb of a wind blowing straight down, whose staff runs up from the
    /// sample, with its side towards +x.
    fn wind(speed: f64) -> (Vec<Segment>, Vec<Flag>) {
        let Barb::Wind { lines, flags } = Barb::new(0.0, -speed, 24.0, false) else {
            panic!("a wind");
        };
        (lines, flags)
    }

    #[test]
    fn the_staff_runs_from_the_sample_against_the_wind() {
        let (lines, _) = wind(10.0);
        assert_eq!(lines[0], [[0.0, 0.0], [0.0, 24.0]]);
        let Barb::Wind { lines, .. } = Barb::new(10.0, 0.0, 24.0, false) else {
            panic!("a wind");
        };
        assert_eq!(lines[0][1], [-24.0, 0.0]);
    }

    #[test]
    fn a_full_line_starts_at_the_end_and_a_half_line_is_half_as_long() {
        let (lines, _) = wind(15.0);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1], [[0.0, 24.0], [LINE_RISE * 24.0, 27.0]]);
        assert_eq!(lines[2], [[0.0, 21.0], [LINE_RISE * 12.0, 22.5]]);
    }

    #[test]
    fn a_half_line_alone_is_set_in_from_the_end() {
        let (lines, flags) = wind(5.0);
        assert!(flags.is_empty());
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1], [[0.0, 21.0], [LINE_RISE * 12.0, 22.5]]);
    }

    #[test]
    fn lines_follow_the_flags_a_step_in() {
        let (lines, flags) = wind(65.0);
        assert_eq!(
            flags,
            [[[0.0, 24.0], [LINE_RISE * 24.0, 24.0], [0.0, 18.0]]]
        );
        assert_eq!(lines[1][0], [0.0, 15.0]);
        assert_eq!(lines[2][0], [0.0, 12.0]);
    }

    #[test]
    fn lines_lie_right_of_the_staff_and_flip_moves_them_left() {
        let side = |flip| {
            let Barb::Wind { lines, .. } = Barb::new(0.0, -10.0, 24.0, flip) else {
                panic!("a wind");
            };
            lines[1][1][0]
        };
        // Looking up the staff, right is +x.
        assert!(side(false) > 0.0);
        assert_eq!(side(true), -side(false));
    }

    #[test]
    fn flags_that_would_pass_the_sample_lengthen_the_staff() {
        let (lines, flags) = wind(250.0);
        assert_eq!(flags.len(), 5);
        assert_eq!(lines[0][1], [0.0, 24.0 * (5.0 * 0.25 + 0.125)]);
        let inner = flags.last().map(|flag| flag[2][1]);
        assert_eq!(inner, Some(3.0));
    }

    #[test]
    fn an_enormous_speed_stays_bounded() {
        let (_, flags) = wind(1e300);
        assert_eq!(flags.len(), MAX_FLAGS as usize);
    }

    #[test]
    fn the_reach_holds_every_stroke() {
        let reach = Barb::new(0.0, -10.0, 24.0, false).reach(1.0);
        assert_eq!(
            reach,
            Reach {
                left: 0.5,
                right: LINE_RISE * 24.0 + 0.5,
                down: 0.5,
                up: 27.5
            }
        );
    }
}
