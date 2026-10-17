//! Sparse rectangular and pointy-top hexagonal counts. Empty cells are never stored.

use std::collections::BTreeMap;

use crate::binning::BinCount;
use crate::{ContinuousScale, MAX_MAGNITUDE, MIN_LOG_VALUE, Transform};

#[cfg(test)]
thread_local! {
    pub(crate) static COUNT_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The range of paired finite coordinates, expanded when it is a single value.
pub(crate) fn ranges(x: &[f64], y: &[f64], transforms: [Transform; 2]) -> Option<[(f64, f64); 2]> {
    let mut extent: Option<[(f64, f64); 2]> = None;
    for (&x, &y) in x
        .iter()
        .zip(y)
        .filter(|(x, y)| x.is_finite() && y.is_finite())
    {
        let e = extent.get_or_insert([(x, x), (y, y)]);
        for (range, v) in e.iter_mut().zip([x, y]) {
            range.0 = range.0.min(v);
            range.1 = range.1.max(v);
        }
    }
    extent.map(|extent| {
        std::array::from_fn(|i| {
            let (lo, hi) = extent[i];
            if lo != hi {
                return (lo, hi);
            }
            match transforms[i] {
                Transform::Log(base) => {
                    let factor = base.get().max(1.1);
                    (
                        (lo / factor).max(MIN_LOG_VALUE),
                        (hi * factor).min(MAX_MAGNITUDE),
                    )
                }
                Transform::Linear => {
                    let half = 0.5_f64.max(lo.abs() * 1e-9);
                    (
                        (lo - half).max(-MAX_MAGNITUDE),
                        (hi + half).min(MAX_MAGNITUDE),
                    )
                }
            }
        })
    })
}

/// Equal-width edges in axis space, with both outer edges exact.
fn edges((lo, hi): (f64, f64), bins: usize, transform: Transform) -> Vec<f64> {
    let width = (hi - lo) / bins as f64;
    (0..=bins)
        .map(|i| {
            if i == 0 {
                return lo;
            }
            if i == bins {
                return hi;
            }
            match transform {
                Transform::Linear => lo + i as f64 * width,
                Transform::Log(_) => log_value(lo, hi, i as f64 / bins as f64),
            }
        })
        .collect()
}

/// Interpolation in logarithmic space, including small relative spans.
fn log_value(lo: f64, hi: f64, t: f64) -> f64 {
    let delta = t * crate::scale::log_offset(hi, lo);
    if delta.abs() < 1.0 {
        lo + lo * delta.exp_m1()
    } else {
        (lo.ln() + delta).exp()
    }
}

/// The half-open bin holding `value`; the final bin also holds the high edge.
fn index(edges: &[f64], value: f64, transform: Transform) -> usize {
    if matches!(transform, Transform::Log(_)) {
        let bins = edges.len() - 1;
        let position = crate::scale::log_offset(value, edges[0])
            / crate::scale::log_offset(edges[bins], edges[0]);
        return ((position * bins as f64).floor() as usize).min(bins - 1);
    }
    edges
        .partition_point(|edge| *edge <= value)
        .saturating_sub(1)
        .min(edges.len().saturating_sub(2))
}

/// Non-empty rectangular cells, in row-major order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RectBins {
    /// Horizontal edges.
    pub x_edges: Vec<f64>,
    /// Vertical edges.
    pub y_edges: Vec<f64>,
    /// Column and row of each non-empty cell.
    pub cells: Vec<(usize, usize)>,
    /// Positive counts, aligned with `cells`.
    pub counts: Vec<f64>,
}

impl RectBins {
    /// Counts paired finite points in equal-width bins over their coordinate ranges.
    pub fn new(x: &[f64], y: &[f64], bins: [BinCount; 2], transforms: [Transform; 2]) -> Self {
        let Some([xr, yr]) = ranges(x, y, transforms) else {
            return Self {
                x_edges: Vec::new(),
                y_edges: Vec::new(),
                cells: Vec::new(),
                counts: Vec::new(),
            };
        };
        let x_edges = edges(xr, bins[0].get(), transforms[0]);
        let y_edges = edges(yr, bins[1].get(), transforms[1]);
        let mut counts = BTreeMap::<(usize, usize), u64>::new();
        for (&x, &y) in x
            .iter()
            .zip(y)
            .filter(|(x, y)| x.is_finite() && y.is_finite())
        {
            *counts
                .entry((
                    index(&y_edges, y, transforms[1]),
                    index(&x_edges, x, transforms[0]),
                ))
                .or_default() += 1;
        }
        let (cells, counts) = counts
            .into_iter()
            .map(|((row, col), n)| ((col, row), n as f64))
            .unzip();
        Self {
            x_edges,
            y_edges,
            cells,
            counts,
        }
    }
}

/// One coordinate mapped to lattice units, with a stable origin on a log axis.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Coordinate {
    scale: ContinuousScale,
    length: f64,
}

impl Coordinate {
    /// Maps a value into lattice units.
    fn at(self, value: f64) -> f64 {
        self.scale.normalize(value) * self.length
    }

    /// Maps a lattice coordinate back to a data value.
    fn value(self, at: f64) -> f64 {
        let (lo, hi) = self.scale.domain();
        let t = at / self.length;
        match self.scale.transform() {
            Transform::Linear => lo + t * (hi - lo),
            Transform::Log(_) => log_value(lo, hi, t),
        }
    }
}

/// Cell geometry before layout, empty data, or a counted lattice.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum HexBins {
    /// The first layout has not chosen the row spacing yet.
    Uncounted,
    /// No paired finite coordinates.
    Empty,
    /// Positive counts with a concrete coordinate map.
    Occupied(HexCells),
}

/// Counted cells. Fitting changes their domains, never their coordinate map or assignments.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HexCells {
    coordinates: [Coordinate; 2],
    /// The row and column of each cell in the lattice, row-major.
    pub positions: Vec<[i64; 2]>,
    /// Positive counts, aligned with `positions`.
    pub counts: Vec<f64>,
    /// Whole-cell limits in data coordinates, after aspect correction.
    pub domains: [[f64; 2]; 2],
    /// Tight bounds in lattice units, before aspect correction.
    bounds: [f64; 4],
}

/// The lattice positions of the six cells around the cell at `[row, col]`, in the order of
/// its edges: the cell across the edge from vertex `i` to vertex `i + 1` of [`hex_vertices`]
/// comes `i`-th (the cells to the upper right, right, lower right, lower left, left and upper
/// left). Rows alternate their offset: an odd row is shifted half a cell to the right.
pub(crate) fn hex_neighbours([row, col]: [i64; 2]) -> [[i64; 2]; 6] {
    // The column of the cell above or below to the right; the one to the left is one less.
    let next = col.saturating_add(row.rem_euclid(2));
    let (up, down) = (row.saturating_add(1), row.saturating_sub(1));
    [
        [up, next],
        [row, col.saturating_add(1)],
        [down, next],
        [down, next.saturating_sub(1)],
        [row, col.saturating_sub(1)],
        [up, next.saturating_sub(1)],
    ]
}

/// The center, in lattice units, of the cell at `[row, col]`: rows are `sqrt(3) / 2` apart and
/// an odd row is shifted half a cell to the right.
pub(crate) fn hex_center([row, col]: [i64; 2]) -> [f64; 2] {
    [
        col as f64 + row.rem_euclid(2) as f64 * 0.5,
        row as f64 * (3.0_f64.sqrt() / 2.0),
    ]
}

/// Vertices of a unit-width pointy-top hexagon, counterclockwise.
pub(crate) fn hex_vertices([x, y]: [f64; 2]) -> [[f64; 2]; 6] {
    let r = 1.0 / 3.0_f64.sqrt();
    [
        [x, y + r],
        [x + 0.5, y + r / 2.0],
        [x + 0.5, y - r / 2.0],
        [x, y - r],
        [x - 0.5, y - r / 2.0],
        [x - 0.5, y + r / 2.0],
    ]
}

impl HexBins {
    /// Data-coordinate metrics shared by both lattice constructors.
    fn coordinates(
        x: &[f64],
        y: &[f64],
        gridsize: BinCount,
        transforms: [Transform; 2],
    ) -> Option<[Coordinate; 2]> {
        ranges(x, y, transforms).map(|ranges| {
            std::array::from_fn(|i| Coordinate {
                scale: ContinuousScale::from_extent(
                    transforms[i],
                    Some(ranges[i]),
                    "Figure.to_scene()",
                )
                .unwrap_or_else(|_| ContinuousScale::unit(transforms[i])),
                length: gridsize.get() as f64,
            })
        })
    }

    /// Bins independently, choosing the lattice rows once from the first solved plot aspect.
    pub fn new(
        x: &[f64],
        y: &[f64],
        gridsize: BinCount,
        transforms: [Transform; 2],
        aspect: f64,
    ) -> Self {
        let Some(mut coordinates) = Self::coordinates(x, y, gridsize, transforms) else {
            return Self::Empty;
        };
        let row_height = 3.0_f64.sqrt() / 2.0;
        let ny = (coordinates[0].length / (aspect * row_height))
            .round()
            .max(1.0);
        coordinates[1].length = ny * row_height;
        let mut bins = Self::count(x, y, coordinates);
        bins.fit(aspect);
        bins
    }

    /// Counts in the metric of the solved axes, including shared or explicit limits.
    pub fn on_axes(
        x: &[f64],
        y: &[f64],
        gridsize: BinCount,
        scales: [ContinuousScale; 2],
        aspect: f64,
        limits: [Option<(f64, f64)>; 2],
    ) -> Self {
        let Some(mut coordinates) =
            Self::coordinates(x, y, gridsize, scales.map(|s| s.transform()))
        else {
            return Self::Empty;
        };
        let span = |scale: ContinuousScale| {
            let (lo, hi) = scale.domain();
            match scale.transform() {
                Transform::Linear => hi - lo,
                Transform::Log(_) => crate::scale::log_offset(hi, lo),
            }
        };
        let relative: [f64; 2] =
            std::array::from_fn(|i| span(coordinates[i].scale) / span(scales[i]));
        coordinates[1].length *= relative[1] / relative[0] / aspect;
        if limits[0].is_some() && limits[1].is_none() {
            // Reserve the end caps before fixing rows; a window shorter than one cell
            // is rejected by fitting, rather than changing its assignments on every pass.
            coordinates[1].length = (coordinates[1].length - 2.0).max(0.1);
        } else if limits[0].is_none() && limits[1].is_some() {
            // The automatic x domain must also leave room for both column end caps.
            coordinates[1].length += 2.0 * relative[1] / aspect;
        }
        Self::count(x, y, coordinates)
    }

    /// Counts against a fixed lattice and computes its occupied bounds.
    fn count(x: &[f64], y: &[f64], coordinates: [Coordinate; 2]) -> Self {
        #[cfg(test)]
        COUNT_CALLS.with(|calls| calls.set(calls.get() + 1));
        let row_height = 3.0_f64.sqrt() / 2.0;
        let mut counts = BTreeMap::<(i64, i64), u64>::new();
        for (&x, &y) in x
            .iter()
            .zip(y)
            .filter(|(x, y)| x.is_finite() && y.is_finite())
        {
            let (x, y) = (coordinates[0].at(x), coordinates[1].at(y));
            let row = (y / row_height).floor() as i64;
            let mut nearest = (0, 0);
            let mut distance = f64::INFINITY;
            for row in row..=row.saturating_add(1) {
                let shift = (row.rem_euclid(2) as f64) * 0.5;
                let col = (x - shift).round() as i64;
                let d = (x - (col as f64 + shift)).powi(2) + (y - row as f64 * row_height).powi(2);
                if d < distance {
                    distance = d;
                    nearest = (row, col);
                }
            }
            *counts.entry(nearest).or_default() += 1;
        }
        let (positions, counts): (Vec<_>, Vec<_>) = counts
            .into_iter()
            .map(|((row, col), count)| ([row, col], count as f64))
            .unzip();
        let bounds = positions
            .iter()
            .flat_map(|&p| hex_vertices(hex_center(p)))
            .fold(
                [
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                ],
                |b, [x, y]| [b[0].min(x), b[1].max(x), b[2].min(y), b[3].max(y)],
            );
        Self::Occupied(HexCells {
            coordinates,
            positions,
            counts,
            bounds,
            domains: [
                [
                    coordinates[0].value(bounds[0]),
                    coordinates[0].value(bounds[1]),
                ],
                [
                    coordinates[1].value(bounds[2]),
                    coordinates[1].value(bounds[3]),
                ],
            ],
        })
    }

    /// The positive counts; empty until a lattice has been counted.
    pub fn counts(&self) -> &[f64] {
        match self {
            Self::Uncounted | Self::Empty => &[],
            Self::Occupied(cells) => &cells.counts,
        }
    }

    /// The fitted bounds of an axis; empty until a lattice has been counted.
    pub fn domain(&self, axis: usize) -> &[f64] {
        match self {
            Self::Uncounted | Self::Empty => &[],
            Self::Occupied(cells) => &cells.domains[axis],
        }
    }

    /// Fits whole cells independently, adding the least symmetric room on just one axis.
    pub fn fit(&mut self, aspect: f64) {
        if let Self::Occupied(cells) = self {
            cells.fit_bounds(cells.bounds, aspect);
        }
    }

    /// Fits a fixed lattice around the current group domain, retaining explicit windows.
    pub fn fit_on_axes(
        &mut self,
        aspect: f64,
        scales: [ContinuousScale; 2],
        limits: [Option<(f64, f64)>; 2],
        context: &'static str,
    ) -> crate::Result<()> {
        let Self::Occupied(cells) = self else {
            return Ok(());
        };
        let c = cells.coordinates;
        let mut bounds = cells.bounds;
        for i in 0..2 {
            let (lo, hi) = scales[i].domain();
            if limits[i].is_none() {
                bounds[2 * i] = bounds[2 * i].min(c[i].at(lo));
                bounds[2 * i + 1] = bounds[2 * i + 1].max(c[i].at(hi));
            }
        }
        match limits {
            [Some(x), Some(y)] => {
                cells.domains = [[x.0, x.1], [y.0, y.1]];
            }
            [Some((lo, hi)), None] => {
                let height = (c[0].at(hi) - c[0].at(lo)) / aspect;
                if height < cells.bounds[3] - cells.bounds[2] - 1e-9 {
                    return Err(window_error(
                        context,
                        "set ylim(), or choose a larger gridsize.",
                    ));
                }
                let mid = (cells.bounds[2] + cells.bounds[3]) / 2.0;
                cells.domains = [
                    [lo, hi],
                    [
                        c[1].value(mid - height / 2.0),
                        c[1].value(mid + height / 2.0),
                    ],
                ];
            }
            [None, Some((lo, hi))] => {
                let width = (c[1].at(hi) - c[1].at(lo)) * aspect;
                if width < cells.bounds[1] - cells.bounds[0] - 1e-9 {
                    return Err(window_error(
                        context,
                        "set xlim(), or choose a larger gridsize.",
                    ));
                }
                let mid = (cells.bounds[0] + cells.bounds[1]) / 2.0;
                cells.domains = [
                    [c[0].value(mid - width / 2.0), c[0].value(mid + width / 2.0)],
                    [lo, hi],
                ];
            }
            [None, None] => cells.fit_bounds(bounds, aspect),
        }
        Ok(())
    }
}

/// A fixed window can be too small for a whole row or column of regular cells.
fn window_error(context: &'static str, hint: &'static str) -> crate::Error {
    crate::Error::data(
        context,
        "hexagons of this size do not fit the window; the automatic axis must contain whole cells.",
    )
    .with_parameter("gridsize")
    .with_hint(hint)
}

impl HexCells {
    /// Fits the least symmetric extra room around bounds expressed in lattice units.
    fn fit_bounds(&mut self, [mut left, mut right, mut bottom, mut top]: [f64; 4], aspect: f64) {
        let width = right - left;
        let height = top - bottom;
        if width / height < aspect {
            let extra = (height * aspect - width) / 2.0;
            left -= extra;
            right += extra;
        } else {
            let extra = (width / aspect - height) / 2.0;
            bottom -= extra;
            top += extra;
        }
        let c = self.coordinates;
        self.domains = [
            [c[0].value(left), c[0].value(right)],
            [c[1].value(bottom), c[1].value(top)],
        ];
    }

    /// The six vertices, in data coordinates, of the cell at lattice `position`, in the order
    /// of [`hex_vertices`].
    pub fn vertices(&self, position: [i64; 2]) -> [[f64; 2]; 6] {
        let c = self.coordinates;
        hex_vertices(hex_center(position)).map(|[x, y]| [c[0].value(x), c[1].value(y)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window_bins(limits: [Option<(f64, f64)>; 2]) -> (HexBins, [ContinuousScale; 2]) {
        let scales = [(-2.0, 4.0), (-1.0, 5.0)]
            .map(|(lo, hi)| ContinuousScale::new(Transform::Linear, lo, hi, "hexbin").unwrap());
        let bins = HexBins::on_axes(
            &[0.0, 1.0, 2.0],
            &[0.0, 2.0, 4.0],
            BinCount::new(10).unwrap(),
            scales,
            1.0,
            limits,
        );
        (bins, scales)
    }

    #[test]
    fn fitting_hexbin_windows_preserves_counted_coordinates() {
        for xlim in [None, Some((-2.0, 4.0))] {
            let limits = [xlim, Some((-1.0, 5.0))];
            let (mut bins, scales) = window_bins(limits);
            let HexBins::Occupied(before) = bins.clone() else {
                panic!("nonempty cells");
            };
            bins.fit_on_axes(1.2, scales, limits, "hexbin").unwrap();
            let HexBins::Occupied(after) = bins else {
                panic!("nonempty cells");
            };
            assert_eq!(after.coordinates, before.coordinates);
            assert_eq!(after.positions, before.positions);
            assert_eq!(after.counts, before.counts);
            for &position in &before.positions {
                assert_eq!(after.vertices(position), before.vertices(position));
            }
            if xlim.is_none() {
                let c = after.coordinates;
                let [[lo, hi], [bottom, top]] = after.domains;
                assert!(
                    (c[0].at(hi) - c[0].at(lo) - (c[1].at(top) - c[1].at(bottom)) * 1.2).abs()
                        < 1e-10
                );
                assert!(
                    (c[0].at(lo) + c[0].at(hi) - after.bounds[0] - after.bounds[1]).abs() < 1e-10
                );
            }
        }
    }

    #[test]
    fn hexbin_ylim_window_rejects_narrow_automatic_x() {
        let limits = [None, Some((-1.0, 5.0))];
        let (mut bins, scales) = window_bins(limits);
        let before = bins.clone();
        let error = bins
            .fit_on_axes(0.01, scales, limits, "hexbin")
            .unwrap_err();
        assert_eq!(error.parameter(), Some("gridsize"));
        assert!(
            error
                .to_string()
                .contains("hexagons of this size do not fit the window")
        );
        assert_eq!(
            error.hint(),
            Some("set xlim(), or choose a larger gridsize.")
        );
        assert_eq!(bins, before);
    }
}
