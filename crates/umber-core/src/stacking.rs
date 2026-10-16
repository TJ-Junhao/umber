//! Stacking: the cumulative segments of stacked series per category, with positive and
//! negative values stacked separately away from zero.

use crate::model::SharedValues;

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

/// The running totals of one stack, one per side of zero.
///
/// Heights are put on the stack in series order: a positive height extends the positive side
/// upward, a negative one the negative side downward. [`Stacks`] builds its segments with it,
/// and the model checks stacked totals with it, so both sum in the same order.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Totals {
    /// The sum of the positive heights so far.
    positive: f64,
    /// The sum of the negative heights so far.
    negative: f64,
}

impl Totals {
    /// Puts `height` on its side of the stack and returns the span it covers, `(bottom, top)`,
    /// with `bottom` the end nearer zero. `None` when it spans nothing: NaN, 0, or a height too
    /// small to change its side's total (`1e100 + 1.0 == 1e100`), which leaves the totals as
    /// they are.
    pub(crate) fn push(&mut self, height: f64) -> Option<(f64, f64)> {
        let side = if height > 0.0 {
            &mut self.positive
        } else if height < 0.0 {
            &mut self.negative
        } else {
            return None;
        };
        let (bottom, top) = (*side, *side + height);
        if top == bottom {
            return None;
        }
        *side = top;
        Some((bottom, top))
    }

    /// The sum of the positive heights put so far; 0 when there is none.
    pub(crate) fn positive(self) -> f64 {
        self.positive
    }

    /// The sum of the negative heights put so far; 0 when there is none.
    pub(crate) fn negative(self) -> f64 {
        self.negative
    }
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
    /// order (see [`Totals`]). NaN and 0 contribute no segment, and neither does a height
    /// missing from a short series. A height too small to change the running total
    /// (`1e100 + 1.0 == 1e100`) spans nothing, so it gets no segment either, and the last
    /// segment that is visible on its side is the outermost. Sums are `f64`; the caller keeps
    /// the totals within the scale limits.
    pub(crate) fn new(categories: usize, series: &[&[f64]]) -> Stacks {
        let mut rows: Vec<Box<[Option<Segment>]>> = series
            .iter()
            .map(|_| vec![None; categories].into_boxed_slice())
            .collect();
        for category in 0..categories {
            let mut totals = Totals::default();
            // The last series that contributed to the positive and the negative side.
            let mut last = [None::<usize>; 2];
            for (index, heights) in series.iter().enumerate() {
                let height = heights.get(category).copied().unwrap_or(f64::NAN);
                let Some((bottom, top)) = totals.push(height) else {
                    continue;
                };
                if let Some(side) = last.get_mut(usize::from(top < 0.0)) {
                    *side = Some(index);
                }
                if let Some(slot) = rows.get_mut(index).and_then(|row| row.get_mut(category)) {
                    *slot = Some(Segment {
                        bottom,
                        top,
                        outermost: false,
                    });
                }
            }
            for index in last.into_iter().flatten() {
                if let Some(Some(segment)) =
                    rows.get_mut(index).and_then(|row| row.get_mut(category))
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

    /// The end away from zero of every segment, series by series and within a series in
    /// category order. These are the values a stack reaches besides 0, which is never among
    /// them: a segment spans a non-zero height away from it. When every height is positive, as
    /// on a log axis, they are exactly the positive segment ends.
    pub(crate) fn ends(&self) -> impl Iterator<Item = f64> + '_ {
        self.rows
            .iter()
            .flat_map(|row| row.iter().flatten().map(|segment| segment.top))
    }
}

/// The cumulative upper curves of non-negative area layers, in series order.
/// Unlike bars, a gap in any series breaks every curve at the same x.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AreaStacks {
    /// One cumulative curve per series, each as long as x.
    curves: Vec<Box<[f64]>>,
}

impl AreaStacks {
    /// Stacks validated non-negative series over x, using the same sums as validation. A gap
    /// in x or a NaN in any series leaves every curve without a value at that index.
    pub(crate) fn new(x: &SharedValues, series: &[&[f64]]) -> AreaStacks {
        let mut curves: Vec<_> = series
            .iter()
            .map(|_| vec![f64::NAN; x.len()].into_boxed_slice())
            .collect();
        for i in 0..x.len() {
            if x.is_gap(i) || series.iter().any(|values| values[i].is_nan()) {
                continue;
            }
            let mut total = Totals::default();
            for (curve, values) in curves.iter_mut().zip(series) {
                total.push(values[i]);
                curve[i] = total.positive();
            }
        }
        AreaStacks { curves }
    }

    /// Every cumulative upper curve, in series order.
    pub(crate) fn curves(&self) -> &[Box<[f64]>] {
        &self.curves
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// The lowest and highest value any stack reaches, 0 included, or `None` without segments.
    fn extent(stacks: &Stacks) -> Option<(f64, f64)> {
        stacks.ends().fold(None, |extent, end| {
            let (lo, hi) = extent.unwrap_or((0.0_f64, 0.0_f64));
            Some((lo.min(end), hi.max(end)))
        })
    }

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
        assert_eq!(extent(&stacks), Some((-4.0, 5.0)));
        assert_eq!(
            stacks.ends().collect::<Vec<_>>(),
            [1.0, 2.0, -1.0, 5.0, 3.0, -4.0]
        );
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
        assert_eq!(extent(&Stacks::new(3, &[])), None);
        assert_eq!(extent(&Stacks::new(1, &[&[0.0]])), None);
    }

    #[test]
    fn totals_sum_each_side_in_order() {
        let mut totals = Totals::default();
        assert_eq!(totals.push(2.0), Some((0.0, 2.0)));
        assert_eq!(totals.push(-1.5), Some((0.0, -1.5)));
        assert_eq!(totals.push(f64::NAN), None);
        assert_eq!(totals.push(0.0), None);
        assert_eq!(totals.push(-0.0), None);
        assert_eq!(totals.push(3.0), Some((2.0, 5.0)));
        assert_eq!((totals.positive(), totals.negative()), (5.0, -1.5));
        // A height that cannot move its side's total spans nothing and changes nothing.
        let mut big = Totals::default();
        big.push(1e100);
        assert_eq!(big.push(1.0), None);
        assert_eq!(big.positive(), 1e100);
    }

    #[test]
    fn a_height_that_cannot_move_the_total_adds_no_segment() {
        for sign in [1.0, -1.0] {
            let (big, tiny) = ([sign * 1e100], [sign * 1.0]);
            let stacks = Stacks::new(1, &[&big, &tiny]);
            let visible = stacks.segment(0, 0).unwrap();
            assert_eq!((visible.bottom, visible.top), (0.0, sign * 1e100));
            assert!(visible.outermost);
            assert!(stacks.segment(0, 1).is_none());
        }
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
                    let side = usize::from(height < 0.0);
                    if previous[side] + height == previous[side] {
                        prop_assert!(segment.is_none());
                        continue;
                    }
                    let segment = segment.unwrap();
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
