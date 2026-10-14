//! The grid of a figure: its rows and columns, how its axes share their scales and how its
//! legend is shown, and the positions axes take on it.

use crate::{Error, Result};

/// The most rows, and the most columns, a figure's grid can have.
///
/// # Examples
///
/// ```
/// use umber_core::{Grid, MAX_GRID};
///
/// assert!(Grid::new(1, MAX_GRID as i64).is_ok());
/// assert!(Grid::new(1, MAX_GRID as i64 + 1).is_err());
/// ```
pub const MAX_GRID: usize = 32;

/// Which axes of a grid share a scale along one dimension: one domain, one tick set and one
/// set of explicit limits, ticks and tick formats. Shared x labels appear only at the lowest
/// occupied panel of each column for `Col` and `All`; shared y labels only at the leftmost
/// occupied panel of each row for `Row` and `All`. Axis titles stay on every panel.
///
/// # Examples
///
/// ```
/// use umber_core::{Grid, Share};
///
/// let grid = Grid::new(2, 2)?.with_share_x(Share::Col);
/// assert_eq!((grid.share_x(), grid.share_y()), (Share::Col, Share::None));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Share {
    /// Every axes has a scale of its own.
    #[default]
    None,
    /// Axes in the same row share.
    Row,
    /// Axes in the same column share.
    Col,
    /// Every axes of the figure shares.
    All,
}

/// How a grid of several cells shows the legends of its axes.
///
/// # Examples
///
/// ```
/// use umber_core::{Grid, LegendMode};
///
/// assert_eq!(Grid::new(1, 2)?.legend(), LegendMode::Merged);
/// let grid = Grid::new(1, 2)?.with_legend(LegendMode::PerPanel);
/// assert_eq!(grid.legend(), LegendMode::PerPanel);
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LegendMode {
    /// One legend for the whole figure.
    #[default]
    Merged,
    /// One legend per axes.
    PerPanel,
}

/// A row or column index on a grid, counting from 0, or `Auto`: index 0 on a dimension of
/// size 1, and an error on a larger one.
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, GridIndex, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
/// assert!(figure.axes_at(GridIndex::Auto, GridIndex::At(0)).is_ok());
/// assert!(figure.axes_at(GridIndex::At(1), GridIndex::Auto).is_err());
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GridIndex {
    /// Index 0 on a dimension of size 1; an error on a larger one.
    #[default]
    Auto,
    /// This index, counting from 0.
    At(i64),
}

/// The grid of a figure: `rows × cols` equal cells, each holding at most one axes, with the
/// sharing of the x and y scales and the legend mode.
///
/// # Examples
///
/// ```
/// use umber_core::{Grid, LegendMode, Share};
///
/// let grid = Grid::new(2, 3)?
///     .with_share_x(Share::Col)
///     .with_share_y(Share::Row)
///     .with_legend(LegendMode::PerPanel);
/// assert_eq!((grid.rows(), grid.cols()), (2, 3));
/// assert_eq!(grid.auto_size_in(), (18.0, 8.0));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Grid {
    rows: usize,
    cols: usize,
    share_x: Share,
    share_y: Share,
    legend: LegendMode,
}

impl Grid {
    /// The grid of a figure with one axes: one row and one column, nothing shared, the
    /// legend merged.
    pub(crate) const SINGLE: Grid = Grid {
        rows: 1,
        cols: 1,
        share_x: Share::None,
        share_y: Share::None,
        legend: LegendMode::Merged,
    };

    /// A grid of `rows × cols` cells, nothing shared and the legend merged. `Error::Style` in
    /// `"Figure()"` unless each of `rows` and `cols` is from 1 to [`MAX_GRID`]; `rows` is
    /// checked first.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Grid;
    ///
    /// assert!(Grid::new(4, 5).is_ok());
    /// let err = Grid::new(0, 2).unwrap_err();
    /// assert_eq!(err.to_string(), "Figure(): rows is 0; rows must be between 1 and 32.");
    /// assert_eq!(err.hint(), Some("use at least 1 row."));
    /// ```
    pub fn new(rows: i64, cols: i64) -> Result<Grid> {
        let rows = grid_size(rows, "rows", "use at least 1 row.")?;
        let cols = grid_size(cols, "cols", "use at least 1 column.")?;
        Ok(Grid {
            rows,
            cols,
            ..Grid::SINGLE
        })
    }

    /// This grid with the x scales shared as `share` says.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Grid, Share};
    ///
    /// assert_eq!(Grid::new(2, 1)?.with_share_x(Share::All).share_x(), Share::All);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn with_share_x(self, share: Share) -> Grid {
        Grid {
            share_x: share,
            ..self
        }
    }

    /// This grid with the y scales shared as `share` says.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Grid, Share};
    ///
    /// assert_eq!(Grid::new(1, 2)?.with_share_y(Share::Row).share_y(), Share::Row);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn with_share_y(self, share: Share) -> Grid {
        Grid {
            share_y: share,
            ..self
        }
    }

    /// This grid with the legend shown as `legend` says.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Grid, LegendMode};
    ///
    /// let grid = Grid::new(2, 2)?.with_legend(LegendMode::PerPanel);
    /// assert_eq!(grid.legend(), LegendMode::PerPanel);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn with_legend(self, legend: LegendMode) -> Grid {
        Grid { legend, ..self }
    }

    /// The number of rows.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(umber_core::Grid::new(3, 1)?.rows(), 3);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// The number of columns.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!(umber_core::Grid::new(3, 1)?.cols(), 1);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// How the x scales are shared; [`Share::None`] unless set.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Grid, Share};
    ///
    /// assert_eq!(Grid::new(2, 2)?.share_x(), Share::None);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn share_x(&self) -> Share {
        self.share_x
    }

    /// How the y scales are shared; [`Share::None`] unless set.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Grid, Share};
    ///
    /// assert_eq!(Grid::new(2, 2)?.share_y(), Share::None);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn share_y(&self) -> Share {
        self.share_y
    }

    /// How the legend is shown; [`LegendMode::Merged`] unless set.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Grid, LegendMode};
    ///
    /// assert_eq!(Grid::new(2, 2)?.legend(), LegendMode::Merged);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn legend(&self) -> LegendMode {
        self.legend
    }

    /// The figure size, in inches, that fits this grid when no size is given: 6 in per column
    /// by 4 in per row, as `(width, height)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Grid;
    ///
    /// assert_eq!(Grid::new(1, 1)?.auto_size_in(), (6.0, 4.0));
    /// assert_eq!(Grid::new(2, 3)?.auto_size_in(), (18.0, 8.0));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn auto_size_in(&self) -> (f64, f64) {
        (6.0 * self.cols as f64, 4.0 * self.rows as f64)
    }

    /// The first `count` cells of the grid in row-major order. `Error::Style` in
    /// `"Figure.cells()"` unless `count` is from 1 to the number of cells.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Cell, Grid};
    ///
    /// let grid = Grid::new(2, 3)?;
    /// let cells = grid.cells(4)?;
    /// assert_eq!(cells[3], Cell { row: 1, col: 0 });
    /// let err = grid.cells(7).unwrap_err();
    /// assert_eq!(err.to_string(), "Figure.cells(): count is 7; a 2 × 3 figure has 6 cells.");
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn cells(&self, count: i64) -> Result<Vec<Cell>> {
        let total = self.rows * self.cols;
        let count = usize::try_from(count)
            .ok()
            .filter(|n| (1..=total).contains(n))
            .ok_or_else(|| {
                Error::style(
                    "Figure.cells()",
                    format!(
                        "count is {count}; a {} × {} figure has {total} cells.",
                        self.rows, self.cols
                    ),
                )
                .with_parameter("count")
                .with_hint("pass a count from 1 to the number of cells.")
            })?;
        Ok((0..count)
            .map(|i| Cell {
                row: i / self.cols,
                col: i % self.cols,
            })
            .collect())
    }
}

/// `value`, the number of rows or columns named `name`, if it is from 1 to [`MAX_GRID`];
/// otherwise the `Error::Style` with `low_hint` below the range.
fn grid_size(value: i64, name: &'static str, low_hint: &'static str) -> Result<usize> {
    usize::try_from(value)
        .ok()
        .filter(|n| (1..=MAX_GRID).contains(n))
        .ok_or_else(|| {
            Error::style(
                "Figure()",
                format!("{name} is {value}; {name} must be between 1 and {MAX_GRID}."),
            )
            .with_parameter(name)
            .with_hint(if value < 1 {
                low_hint
            } else {
                "split the figure into several figures."
            })
        })
}

/// One cell of a grid, counting from 0 at the top-left.
///
/// # Examples
///
/// ```
/// use umber_core::{Cell, Grid};
///
/// let first = Grid::new(2, 2)?.cells(1)?[0];
/// assert_eq!(first, Cell { row: 0, col: 0 });
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cell {
    /// The row.
    pub row: usize,
    /// The column.
    pub col: usize,
}

/// Where to add an axes on a grid: its top-left cell and how many rows and columns it spans.
/// The default is `Auto`, `Auto`, 1, 1: the one cell of a 1 × 1 grid.
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, Grid, GridIndex, Placement, ScaleKind, Theme};
///
/// let mut figure = Figure::with_grid(9.0, 6.0, Theme::pigment(), Grid::new(2, 2)?)?;
/// let wide = Placement {
///     row: GridIndex::At(1),
///     col: GridIndex::At(0),
///     col_span: 2,
///     ..Placement::default()
/// };
/// let axes = figure.add_axes_at(wide, ScaleKind::Linear, ScaleKind::Linear)?;
/// assert_eq!((axes.cells().row(), axes.cells().col_span()), (1, 2));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Placement {
    /// The row of the top-left cell.
    pub row: GridIndex,
    /// The column of the top-left cell.
    pub col: GridIndex,
    /// The number of rows the axes spans, at least 1.
    pub row_span: i64,
    /// The number of columns the axes spans, at least 1.
    pub col_span: i64,
}

impl Default for Placement {
    fn default() -> Placement {
        Placement {
            row: GridIndex::Auto,
            col: GridIndex::Auto,
            row_span: 1,
            col_span: 1,
        }
    }
}

/// The cells an axes covers on its grid: a rectangle of `row_span × col_span` cells whose
/// top-left cell is at `row`, `col`. Every range an axes holds lies on its figure's grid.
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, ScaleKind, Theme};
///
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let cells = figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?.cells();
/// assert_eq!((cells.row(), cells.col(), cells.row_span(), cells.col_span()), (0, 0, 1, 1));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellRange {
    row: usize,
    col: usize,
    row_span: usize,
    col_span: usize,
}

impl CellRange {
    /// The range of `row_span × col_span` cells from `row`, `col`; the caller has checked it
    /// against the grid.
    pub(crate) fn new(row: usize, col: usize, row_span: usize, col_span: usize) -> CellRange {
        CellRange {
            row,
            col,
            row_span,
            col_span,
        }
    }

    /// The row of the top-left cell.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// assert_eq!(figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?.cells().row(), 0);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn row(&self) -> usize {
        self.row
    }

    /// The column of the top-left cell.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// assert_eq!(figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?.cells().col(), 0);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn col(&self) -> usize {
        self.col
    }

    /// The number of rows covered, at least 1.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// assert_eq!(figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?.cells().row_span(), 1);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn row_span(&self) -> usize {
        self.row_span
    }

    /// The number of columns covered, at least 1.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// assert_eq!(figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?.cells().col_span(), 1);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn col_span(&self) -> usize {
        self.col_span
    }

    /// Whether the range covers the cell at `row`, `col`.
    pub(crate) fn contains(&self, row: usize, col: usize) -> bool {
        (self.row..self.row + self.row_span).contains(&row)
            && (self.col..self.col + self.col_span).contains(&col)
    }

    /// The covered cells, in row-major order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = Cell> + use<> {
        let (rows, cols) = (
            self.row..self.row + self.row_span,
            self.col..self.col + self.col_span,
        );
        rows.flat_map(move |row| cols.clone().map(move |col| Cell { row, col }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_checked_rows_first() {
        for (rows, cols, message, parameter, hint) in [
            (
                0,
                0,
                "rows is 0; rows must be between 1 and 32.",
                "rows",
                "use at least 1 row.",
            ),
            (
                33,
                1,
                "rows is 33; rows must be between 1 and 32.",
                "rows",
                "split the figure into several figures.",
            ),
            (
                1,
                -4,
                "cols is -4; cols must be between 1 and 32.",
                "cols",
                "use at least 1 column.",
            ),
            (
                1,
                i64::MAX,
                "cols is 9223372036854775807; cols must be between 1 and 32.",
                "cols",
                "split the figure into several figures.",
            ),
        ] {
            let err = Grid::new(rows, cols).unwrap_err();
            assert!(matches!(err, Error::Style { .. }), "{err:?}");
            assert_eq!(err.to_string(), format!("Figure(): {message}"));
            assert_eq!(err.parameter(), Some(parameter));
            assert_eq!(err.hint(), Some(hint));
        }
        let grid = Grid::new(32, 1).unwrap();
        assert_eq!((grid.rows(), grid.cols()), (32, 1));
        assert_eq!(Grid::new(1, 1).unwrap(), Grid::SINGLE);
    }

    #[test]
    fn cells_are_row_major_and_counted() {
        let grid = Grid::new(2, 3).unwrap();
        let all: Vec<(usize, usize)> = grid
            .cells(6)
            .unwrap()
            .into_iter()
            .map(|c| (c.row, c.col))
            .collect();
        assert_eq!(all, [(0, 0), (0, 1), (0, 2), (1, 0), (1, 1), (1, 2)]);
        assert_eq!(grid.cells(1).unwrap(), [Cell { row: 0, col: 0 }]);
        for count in [0, -1, 7, i64::MIN] {
            let err = grid.cells(count).unwrap_err();
            assert!(matches!(err, Error::Style { .. }), "{err:?}");
            assert_eq!(
                err.to_string(),
                format!("Figure.cells(): count is {count}; a 2 × 3 figure has 6 cells.")
            );
            assert_eq!(err.parameter(), Some("count"));
            assert_eq!(
                err.hint(),
                Some("pass a count from 1 to the number of cells.")
            );
        }
    }

    #[test]
    fn ranges_cover_their_cells() {
        let range = CellRange::new(1, 2, 2, 3);
        let cells: Vec<(usize, usize)> = range.iter().map(|c| (c.row, c.col)).collect();
        assert_eq!(cells, [(1, 2), (1, 3), (1, 4), (2, 2), (2, 3), (2, 4)]);
        assert!(cells.iter().all(|&(r, c)| range.contains(r, c)));
        assert!(!range.contains(0, 2) && !range.contains(1, 5) && !range.contains(3, 3));
    }

    #[test]
    fn defaults() {
        assert_eq!(Share::default(), Share::None);
        assert_eq!(LegendMode::default(), LegendMode::Merged);
        assert_eq!(GridIndex::default(), GridIndex::Auto);
        let placement = Placement::default();
        assert_eq!(
            (
                placement.row,
                placement.col,
                placement.row_span,
                placement.col_span
            ),
            (GridIndex::Auto, GridIndex::Auto, 1, 1)
        );
        assert_eq!(Grid::SINGLE.auto_size_in(), (6.0, 4.0));
    }
}
