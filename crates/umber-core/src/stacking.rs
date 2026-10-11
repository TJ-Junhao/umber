//! Stacking: the cumulative segments of stacked series per category, with positive and
//! negative values stacked separately away from zero.

#![allow(dead_code, reason = "stacked bar marks will use it")]

/// One series' share of a stack: the span from `bottom` to `top`.
///
/// `bottom` is the end nearer zero and `top` the end away from it, so a negative segment has
/// `top < bottom`. Each segment's `bottom` is the `top` of the one before it on the same side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Segment {
    /// The end nearer zero.
    pub(crate) bottom: f64,
    /// The end away from zero.
    pub(crate) top: f64,
    /// Whether this is the last segment on its side of zero, the end that is drawn rounded.
    pub(crate) outermost: bool,
}

/// The segments of every series in every category.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Stacks {
    /// One row of segments per series, one entry per category.
    rows: Vec<Box<[Option<Segment>]>>,
}

impl Stacks {
    /// Stacks `series` (one height per category each) over `categories` categories.
    ///
    /// Positive heights stack upward from 0 and negative ones downward, each side in series
    /// order. NaN and 0 contribute no segment, and neither does a height missing from a short
    /// series. Sums are `f64`; the caller keeps the totals within the scale limits.
    pub(crate) fn new(categories: usize, series: &[&[f64]]) -> Stacks {
        let mut rows: Vec<Box<[Option<Segment>]>> = series
            .iter()
            .map(|_| vec![None; categories].into_boxed_slice())
            .collect();
        for category in 0..categories {
            // Running totals and the last series that contributed, for each side.
            let mut positive = (0.0_f64, None::<usize>);
            let mut negative = (0.0_f64, None::<usize>);
            for (index, heights) in series.iter().enumerate() {
                let height = heights.get(category).copied().unwrap_or(f64::NAN);
                let side = if height > 0.0 {
                    &mut positive
                } else if height < 0.0 {
                    &mut negative
                } else {
                    continue;
                };
                let (bottom, top) = (side.0, side.0 + height);
                side.0 = top;
                side.1 = Some(index);
                if let Some(slot) = rows.get_mut(index).and_then(|row| row.get_mut(category)) {
                    *slot = Some(Segment {
                        bottom,
                        top,
                        outermost: false,
                    });
                }
            }
            for last in [positive.1, negative.1].into_iter().flatten() {
                if let Some(Some(segment)) =
                    rows.get_mut(last).and_then(|row| row.get_mut(category))
                {
                    segment.outermost = true;
                }
            }
        }
        Stacks { rows }
    }

    /// The segment of `series` in `category`, or `None` when it has none.
    pub(crate) fn segment(&self, category: usize, series: usize) -> Option<Segment> {
        self.rows.get(series)?.get(category).copied().flatten()
    }

    /// The lowest and highest value any stack reaches, 0 included, or `None` when no series
    /// has a segment.
    pub(crate) fn extent(&self) -> Option<(f64, f64)> {
        self.rows
            .iter()
            .flat_map(|row| row.iter().flatten())
            .fold(None, |extent, segment| {
                let (lo, hi) = extent.unwrap_or((0.0_f64, 0.0_f64));
                Some((
                    lo.min(segment.bottom).min(segment.top),
                    hi.max(segment.bottom).max(segment.top),
                ))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn positive_and_negative_heights_stack_apart() {
        let (a, b, c) = ([1.0, 2.0], [-1.0, 3.0], [2.0, -4.0]);
        let stacks = Stacks::new(2, &[&a, &b, &c]);
        let seg = |category, series| stacks.segment(category, series).unwrap();

        assert_eq!((seg(0, 0).bottom, seg(0, 0).top), (0.0, 1.0));
        assert_eq!((seg(0, 1).bottom, seg(0, 1).top), (0.0, -1.0));
        assert_eq!((seg(0, 2).bottom, seg(0, 2).top), (1.0, 3.0));
        assert_eq!((seg(1, 1).bottom, seg(1, 1).top), (2.0, 5.0));
        assert_eq!((seg(1, 2).bottom, seg(1, 2).top), (0.0, -4.0));
        assert_eq!(stacks.extent(), Some((-4.0, 5.0)));
    }

    #[test]
    fn only_the_last_segment_on_each_side_is_outermost() {
        let (a, b, c) = ([1.0], [-1.0], [2.0]);
        let stacks = Stacks::new(1, &[&a, &b, &c]);
        let outermost = |series| stacks.segment(0, series).unwrap().outermost;
        assert!(!outermost(0));
        assert!(outermost(1));
        assert!(outermost(2));
    }

    #[test]
    fn nan_and_zero_heights_add_no_segment() {
        let (a, b, c) = ([f64::NAN, 1.0], [0.0, f64::NAN], [2.0, 3.0]);
        let stacks = Stacks::new(2, &[&a, &b, &c]);
        assert!(stacks.segment(0, 0).is_none());
        assert!(stacks.segment(0, 1).is_none());
        assert!(stacks.segment(1, 1).is_none());
        let first = stacks.segment(0, 2).unwrap();
        assert_eq!((first.bottom, first.top, first.outermost), (0.0, 2.0, true));
        let second = stacks.segment(1, 0).unwrap();
        assert_eq!(
            (second.bottom, second.top, second.outermost),
            (0.0, 1.0, false)
        );
    }

    #[test]
    fn out_of_range_lookups_and_empty_input_have_no_segment() {
        let stacks = Stacks::new(1, &[&[1.0]]);
        assert!(stacks.segment(1, 0).is_none());
        assert!(stacks.segment(0, 1).is_none());
        assert_eq!(Stacks::new(3, &[]).extent(), None);
        assert_eq!(Stacks::new(1, &[&[0.0]]).extent(), None);
    }

    #[test]
    fn a_short_series_has_no_segment_past_its_end() {
        let stacks = Stacks::new(2, &[&[1.0]]);
        assert!(stacks.segment(0, 0).is_some());
        assert!(stacks.segment(1, 0).is_none());
    }

    proptest! {
        #[test]
        fn stacks_are_contiguous(
            data in prop::collection::vec(
                prop::collection::vec(
                    prop_oneof![-1e6_f64..1e6, Just(0.0), Just(f64::NAN)],
                    4,
                ),
                0..6,
            ),
        ) {
            let series: Vec<&[f64]> = data.iter().map(Vec::as_slice).collect();
            let stacks = Stacks::new(4, &series);
            for category in 0..4 {
                let mut previous = [0.0_f64; 2];
                let mut last: [Option<usize>; 2] = [None, None];
                for (index, heights) in data.iter().enumerate() {
                    let height = heights[category];
                    let segment = stacks.segment(category, index);
                    if height.is_nan() || height == 0.0 {
                        prop_assert!(segment.is_none());
                        continue;
                    }
                    let segment = segment.unwrap();
                    let side = usize::from(height < 0.0);
                    prop_assert_eq!(segment.bottom, previous[side]);
                    prop_assert_eq!(segment.top, previous[side] + height);
                    previous[side] = segment.top;
                    last[side] = Some(index);
                }
                for (index, _) in data.iter().enumerate() {
                    if let Some(segment) = stacks.segment(category, index) {
                        prop_assert_eq!(segment.outermost, last.contains(&Some(index)));
                    }
                }
            }
        }
    }
}
