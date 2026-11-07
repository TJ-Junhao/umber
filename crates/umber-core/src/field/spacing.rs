//! The typical spacing of samples: what the automatic scale of arrows makes the longest arrow.
//!
//! On a grid that is a median gap between neighbouring coordinates ([`median_gap`]); for points
//! it is the median distance to the nearest neighbour ([`median_nearest`]). Both are measured
//! in points on the plot, so they are computed anew for each plot: the nearest neighbour of a
//! point depends on how the plot stretches x against y.
//!
//! The nearest neighbours come from a k-d tree: building it takes O(n log n), and a search
//! visits O(log n) nodes for points spread over the plot. A node splits along the wider side of
//! the box of its points, so points along one line, the worst case of a fixed order of axes,
//! are as fast as any others.

/// The median of `values`, which it reorders: the middle one, or the mean of the two middle
/// ones; `None` without values.
fn median(values: &mut [f64]) -> Option<f64> {
    let count = values.len();
    if count == 0 {
        return None;
    }
    let middle = count / 2;
    let (below, &mut upper, _) = values.select_nth_unstable_by(middle, f64::total_cmp);
    if count % 2 == 1 {
        return Some(upper);
    }
    let lower = below.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Some(lower / 2.0 + upper / 2.0)
}

/// The median gap between neighbours of the increasing `values`; `None` for fewer than two.
pub(crate) fn median_gap(values: &[f64]) -> Option<f64> {
    let mut gaps: Vec<f64> = values.windows(2).map(|pair| pair[1] - pair[0]).collect();
    median(&mut gaps)
}

/// The median distance from each of the `points` to its nearest neighbour, where points at one
/// place count as one: two samples at a station give the station one position. `None` with
/// fewer than two distinct points.
///
/// The points are finite.
pub(crate) fn median_nearest(mut points: Vec<[f64; 2]>) -> Option<f64> {
    points.sort_unstable_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    points.dedup();
    if points.len() < 2 {
        return None;
    }
    let tree = Tree::new(points);
    let mut distances: Vec<f64> = (0..tree.points.len())
        .map(|i| tree.nearest(i).sqrt())
        .collect();
    median(&mut distances)
}

/// A balanced k-d tree stored in place: the node of a range of `points` is its middle, and
/// the two halves around it are its subtrees.
struct Tree {
    /// The points, in tree order.
    points: Vec<[f64; 2]>,
    /// The axis each node splits along, by the index of its point.
    axes: Vec<u8>,
}

impl Tree {
    /// The tree of `points`.
    fn new(mut points: Vec<[f64; 2]>) -> Tree {
        let mut axes = vec![0; points.len()];
        split(&mut points, &mut axes);
        Tree { points, axes }
    }

    /// The squared distance from the point at `index` to the nearest other point; the points
    /// are distinct.
    fn nearest(&self, index: usize) -> f64 {
        let mut best = f64::INFINITY;
        self.search(0..self.points.len(), index, &mut best);
        best
    }

    /// Lowers `best` to the squared distance from the point at `index` to the nearest other
    /// point of the subtree over `range`, if that is nearer.
    fn search(&self, range: std::ops::Range<usize>, index: usize, best: &mut f64) {
        if range.is_empty() {
            return;
        }
        let node = range.start + range.len() / 2;
        let (point, target) = (self.points[node], self.points[index]);
        if node != index {
            let (dx, dy) = (point[0] - target[0], point[1] - target[1]);
            *best = best.min(dx * dx + dy * dy);
        }
        let axis = usize::from(self.axes[node]);
        let offset = target[axis] - point[axis];
        let (near, far) = if offset < 0.0 {
            (range.start..node, node + 1..range.end)
        } else {
            (node + 1..range.end, range.start..node)
        };
        self.search(near, index, best);
        if offset * offset < *best {
            self.search(far, index, best);
        }
    }
}

/// Arranges `points` as the subtree over them, each node's axis in `axes`.
fn split(points: &mut [[f64; 2]], axes: &mut [u8]) {
    if points.is_empty() {
        return;
    }
    let spread = |axis: usize| {
        let (low, high) = points
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), point| {
                (low.min(point[axis]), high.max(point[axis]))
            });
        high - low
    };
    let axis = usize::from(spread(1) > spread(0));
    let middle = points.len() / 2;
    points.select_nth_unstable_by(middle, |a, b| a[axis].total_cmp(&b[axis]));
    axes[middle] = axis as u8;
    let (low_points, rest) = points.split_at_mut(middle);
    let (low_axes, rest_axes) = axes.split_at_mut(middle);
    split(low_points, low_axes);
    split(&mut rest[1..], &mut rest_axes[1..]);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The median nearest distance by comparing every pair.
    fn by_every_pair(points: &[[f64; 2]]) -> Option<f64> {
        let mut unique = points.to_vec();
        unique.sort_unstable_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
        unique.dedup();
        let mut distances: Vec<f64> = unique
            .iter()
            .enumerate()
            .map(|(i, a)| {
                unique
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, b)| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2))
                    .fold(f64::INFINITY, f64::min)
                    .sqrt()
            })
            .collect();
        median(&mut distances)
    }

    /// Points from a fixed sequence, so that the test needs no random numbers.
    fn scattered(count: u64, stretch: f64) -> Vec<[f64; 2]> {
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f64 / (1_u64 << 53) as f64
        };
        (0..count)
            .map(|_| [next() * 400.0, next() * 300.0 * stretch])
            .collect()
    }

    #[test]
    fn a_median_is_the_middle_or_the_mean_of_the_middle_two() {
        assert_eq!(median(&mut []), None);
        assert_eq!(median(&mut [3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&mut [4.0, 1.0, 2.0, 9.0]), Some(3.0));
    }

    #[test]
    fn gaps_need_two_values() {
        assert_eq!(median_gap(&[1.0]), None);
        assert_eq!(median_gap(&[0.0, 1.0, 2.0, 10.0]), Some(1.0));
    }

    #[test]
    fn the_tree_agrees_with_every_pair() {
        for (count, stretch) in [(2, 1.0), (3, 1.0), (97, 1.0), (500, 0.001), (500, 50.0)] {
            let points = scattered(count, stretch);
            assert_eq!(
                median_nearest(points.clone()),
                by_every_pair(&points),
                "{count}"
            );
        }
    }

    #[test]
    fn points_on_one_line_are_measured_too() {
        let line: Vec<_> = (0..1000).map(|i| [0.0, 2.0 * f64::from(i)]).collect();
        assert_eq!(median_nearest(line), Some(2.0));
    }

    #[test]
    fn points_at_one_place_count_as_one() {
        assert_eq!(median_nearest(vec![[1.0, 1.0]; 5]), None);
        assert_eq!(median_nearest(vec![]), None);
        let doubled = vec![[0.0, 0.0], [0.0, 0.0], [3.0, 4.0], [3.0, 4.0]];
        assert_eq!(median_nearest(doubled), Some(5.0));
    }

    #[test]
    fn a_hundred_thousand_points_take_a_bounded_number_of_visits() {
        // Not a timing: a quadratic scan of this many points would not end within the test run.
        let points = scattered(100_000, 1.0);
        assert!(median_nearest(points).is_some_and(|distance| distance > 0.0));
    }
}
