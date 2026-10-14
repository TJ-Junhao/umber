//! The grid solve: the plot areas of every panel of a figure, chosen at once so that every
//! panel's plot area has the same size and the panels line up.
//!
//! A figure of one cell keeps the layout of a single panel ([`super::solve_single`]): the
//! canvas is its frame, and its header is stacked from the padding across the inner width. On
//! a larger grid, with `gap` the theme's [`Theme::panel_gap`]:
//!
//! 1. Bands. Each column `c` has a band `L[c]` left of its plots, the widest y tick label
//!    column (label and gap) among the panels whose first column is `c`, and a band `R[c]`
//!    right of them, the most room direct labels reserve among the panels whose last column is
//!    `c`. Each row `r` has a band `T[r]` above its plots, the most room above the plot (header,
//!    y label and the rise of the top y tick label) among the panels whose first row is `r`,
//!    and a band `B[r]` below them, the most room below the plot (x tick label row and x label
//!    row, or the bottom y tick label reaching below the plot when that is further) among the
//!    panels whose last row is `r`.
//! 2. Plot sizes. Every column's plot is `P` wide and every row's `Q` tall, with
//!    `P = (inner width − ΣL − ΣR − gap × (cols − 1)) / cols` and `Q` likewise from the inner
//!    height, `T` and `B`; the inner width and height are the canvas inside the padding. A
//!    panel that spans several cells runs from the plot left of its first column to the plot
//!    right of its last, the bands and gaps between them included; likewise down. A column or
//!    row without panels keeps its share, with bands of 0.
//! 3. x tick labels, centered on their ticks. On the canvas edges they may reach into the
//!    padding up to [`X_LABEL_EDGE_GAP`] from the edge, as on a single panel; beyond that the
//!    whole grid moves in on that side, by at most [`END_LABEL_SHARE`] of the inner width
//!    beyond the band that is already there. Between two columns a label may reach into the
//!    gap up to its middle; beyond that, the band of its column on that side grows by the
//!    overflow, for every panel of the column, so the plots stay equal. Growing a band narrows
//!    every plot, and widens the plots of the panels that span across it, so the least growth
//!    that fits every label is solved as a linear program (see [`fit_columns`]).
//! 4. Headers. A panel's header starts at its row's top; its title, subtitle and legend row
//!    start at the plot's left edge, and the legend row wraps within the plot's width. The x
//!    axis labels of a row sit at the bottom of its band below the plots, so they line up.
//! 5. Containment. Once the final layout is placed, every panel's title, subtitle, legend row
//!    entries and axis labels must fit its plot width, and its direct labels its frame (see
//!    [`check_panel_contents`]).
//!
//! Each panel's frame, the part of the canvas that belongs to it, is its plot with its
//! column and row bands, reaching halfway into the gap next to a neighbouring cell and to the
//! canvas edge on the grid's outer sides. Neighbouring frames touch and never overlap, so the
//! marks clip of a panel never reaches into the next cell.

use super::inset::{CONTAINMENT_TOLERANCE, TickSpan};
use super::panel::{HeaderBox, PanelBands, PanelSlot, PanelSpan, end_label_cap};
use super::program;
use super::{
    AXIS_LABEL_GAP, Area, END_LABEL_SHARE, Frame, LEGEND_HINT, Label, Layout, LayoutInput,
    MIN_PLOT, PADDING, PlotSize, X_LABEL_EDGE_GAP, initial_plot_size, solve_single, widest, widths,
    x_labels_wider_than, y_labels_leave_no_room,
};
use crate::error::{number, quoted};
use crate::model::{CellRange, Grid};
use crate::{Error, Result, Theme};

/// The most passes that settle the rounding of a solution of the x tick label program (see
/// [`Columns::settle`]).
const X_LABEL_PASSES: usize = 5;

/// How far, in points, the x tick label program first raises every bound, so that the
/// rounding of its solution stays within the original bounds (see [`fit_columns`]): far below
/// anything visible, and far above the rounding of the solve.
const PROGRAM_MARGIN: f64 = 1e-7;

/// The overflow, in points, below which an x tick label counts as within its bound when the
/// solve decides whether to grow a band: far below anything visible.
const MOVE_TOLERANCE: f64 = 1e-9;

/// The bisection steps that find the widest x tick label that fits a grid; as in
/// [`super::inset::widest_label_that_fits`].
const BISECTION_STEPS: usize = 64;

/// What the grid solve lays out: the canvas, the grid and every panel on it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GridInput<'a> {
    /// The canvas width, in points.
    pub width: f64,
    /// The canvas height, in points.
    pub height: f64,
    /// The top of the grid, below the figure header; the outer padding without a header.
    pub top: f64,
    /// The grid.
    pub grid: &'a Grid,
    /// The theme, for the panel gap.
    pub theme: &'a Theme,
    /// The API call that renders, which layout errors are reported in.
    pub context: &'static str,
    /// Every panel, in the order of `Figure::panels`.
    pub panels: &'a [LayoutInput<'a>],
}

/// Where the grid solve put every panel, in the order of [`GridInput::panels`].
#[derive(Debug, Clone)]
pub(crate) struct GridSolution {
    /// The slot of each panel.
    pub panels: Vec<PanelSlot>,
}

impl GridInput<'_> {
    /// Whether the grid is a single cell, laid out as one panel on the canvas.
    fn is_single(&self) -> bool {
        self.grid.rows() * self.grid.cols() == 1
    }

    /// The gap between neighbouring cells.
    fn gap(&self) -> f64 {
        f64::from(self.theme.panel_gap())
    }

    /// The widths the panel covering `cells` is measured for. A figure of one cell wraps its
    /// legend row within the inner width and caps direct labels at [`END_LABEL_SHARE`] of it.
    /// On a larger grid the cells' share of the inner width stands in for the plot width,
    /// which is not known yet: the solve wraps the row again within the plot (see
    /// [`solve_grid`]), and direct labels are capped at [`END_LABEL_SHARE`] of that share.
    pub fn span(&self, cells: CellRange) -> PanelSpan {
        if self.is_single() {
            return PanelSpan {
                row_left: PADDING,
                row_right: self.width - PADDING,
                reserve_cap: end_label_cap(self.width),
            };
        }
        let (left, width) = cell_extent(
            self.width,
            self.grid.cols(),
            self.gap(),
            cells.col(),
            cells.col_span(),
        );
        PanelSpan {
            row_left: left,
            row_right: left + width,
            reserve_cap: END_LABEL_SHARE * width,
        }
    }

    /// The plot size the panel covering `cells` is assumed to have before any tick label is
    /// measured: the canvas inside the padding for a figure of one cell, and otherwise the
    /// cells' share of it, gaps between them included; at least [`MIN_PLOT`] a side.
    pub fn initial_plot_size(&self, cells: CellRange) -> PlotSize {
        if self.is_single() {
            return initial_plot_size(self.width, self.height - (self.top - PADDING));
        }
        let gap = self.gap();
        let (_, width) = cell_extent(
            self.width,
            self.grid.cols(),
            gap,
            cells.col(),
            cells.col_span(),
        );
        let (_, height) = cell_extent(
            self.height - (self.top - PADDING),
            self.grid.rows(),
            gap,
            cells.row(),
            cells.row_span(),
        );
        PlotSize {
            width: width.max(MIN_PLOT),
            height: height.max(MIN_PLOT),
        }
    }
}

/// Along one side of the canvas, `extent` pt long and split inside the padding into `count`
/// equal cells `gap` apart: where the `span` cells from cell `first` start, and how long they
/// are together, the gaps between them included.
fn cell_extent(extent: f64, count: usize, gap: f64, first: usize, span: usize) -> (f64, f64) {
    let gaps = |n: usize| gap * n as f64;
    let cell = ((extent - 2.0 * PADDING - gaps(count - 1)) / count as f64).max(0.0);
    let start = PADDING + first as f64 * (cell + gap);
    (start, cell * span as f64 + gaps(span - 1))
}

/// `err`, a layout error of the panel covering `cells`, naming the panel's top-left cell first.
fn in_panel(cells: CellRange, err: Error) -> Error {
    err.with_message_prefix(&format!("row {}, col {}: ", cells.row(), cells.col()))
}

/// The plot area, header box and frame of every panel of `input`, for the texts measured in
/// `bands` (one per panel, in the same order); see the module documentation for the rules.
///
/// # Errors
///
/// `Error::Style` in `input.context`. A figure of one cell fails as a single panel does (see
/// [`super::solve_single`]), with the messages of a figure without a grid. A larger grid is
/// checked in this order, the first failure reported:
/// - width: the plots of the columns are narrower than [`MIN_PLOT`] before x tick labels are
///   fitted. When y tick labels of one column, narrower, would leave room, the message names
///   the widest y tick label of the widest such column and its panel (see
///   [`y_labels_leave_no_room`]); otherwise it names the width the figure needs
///   (`the figure is {W} pt wide; at least {need} pt is needed for {cols} columns of plots.`);
/// - x tick labels: no growth of the bands within the caps fits them (see [`fit_columns`]);
///   the message names the panel, its widest x tick label and the width up to which every
///   label of the figure would fit (see [`x_labels_wider_than`]);
/// - height: the plots of the rows are shorter than [`MIN_PLOT`]
///   (`the figure is {H} pt tall; at least {need} pt is needed for {rows} rows of plots.`);
///
/// Texts that span a panel are checked against the final layout by [`check_panel_contents`].
pub(crate) fn solve_grid(input: &GridInput<'_>, bands: &[PanelBands]) -> Result<GridSolution> {
    if input.is_single() {
        let canvas = Frame {
            x: 0.0,
            y: 0.0,
            width: input.width,
            height: input.height,
        };
        let header = HeaderBox {
            left: canvas.x + PADDING,
            right: canvas.right() - PADDING,
            top: input.top,
        };
        let panels = input
            .panels
            .iter()
            .zip(bands)
            .map(|(panel, bands)| {
                solve_single(panel, bands, canvas, input.top).map(|plot| PanelSlot {
                    plot,
                    header,
                    xlabel_baseline: plot.bottom()
                        + bands.x_band
                        + AXIS_LABEL_GAP
                        + bands.body_ascent,
                    frame: canvas,
                })
            })
            .collect::<Result<_>>()?;
        return Ok(GridSolution { panels });
    }
    let cells: Vec<CellRange> = input.panels.iter().map(|p| p.axes.cells()).collect();
    let columns = Columns::measure(input, &cells, bands);
    let open = columns.place(&columns.no_growth());
    if open.plot_width < MIN_PLOT {
        return Err(columns_too_narrow(input, &cells, bands, &columns));
    }
    let bounds = columns.label_bounds(&cells, bands, f64::INFINITY);
    let placed = match fit_columns(&columns, &bounds) {
        Ok(placed) => placed,
        Err(stuck) => return Err(x_labels_do_not_fit(input, &cells, bands, &columns, &stuck)),
    };
    let spans: Vec<(f64, f64)> = cells.iter().map(|c| placed.span(*c)).collect();
    let rows = Rows::measure(input, &cells, bands, &spans);
    let rows = rows.place(input)?;
    let panels = cells
        .iter()
        .zip(bands)
        .zip(&spans)
        .map(|((cells, bands), &(left, right))| {
            let (top, bottom) = rows.span(*cells);
            let last_row = cells.row() + cells.row_span() - 1;
            let (frame_left, frame_right) = placed.frame(*cells, columns.canvas);
            let (frame_top, frame_bottom) = rows.frame(*cells, input.height);
            PanelSlot {
                plot: Area {
                    x: left,
                    y: top,
                    width: right - left,
                    height: bottom - top,
                },
                header: HeaderBox {
                    left,
                    right,
                    top: rows.row_top[cells.row()],
                },
                xlabel_baseline: bottom + (rows.below[last_row] - rows.legends[last_row])
                    - bands.body_descent,
                frame: Frame {
                    x: frame_left,
                    y: frame_top,
                    width: frame_right - frame_left,
                    height: frame_bottom - frame_top,
                },
            }
        })
        .collect();
    Ok(GridSolution { panels })
}

/// The column bands of a grid, as measured, before x tick labels grow any of them.
#[derive(Debug, Clone)]
struct Columns {
    /// The canvas width.
    canvas: f64,
    /// The gap between neighbouring columns.
    gap: f64,
    /// `L[c]`: the band left of each column's plots.
    left: Vec<f64>,
    /// `R[c]`: the band right of each column's plots.
    right: Vec<f64>,
}

/// The columns placed for some growth of their bands.
#[derive(Debug, Clone)]
struct PlacedColumns {
    /// Each column's plot left edge.
    plot_x: Vec<f64>,
    /// The plot width of every column, `P`.
    plot_width: f64,
    /// Each column's left band, grown.
    left: Vec<f64>,
    /// Each column's right band, grown.
    right: Vec<f64>,
}

impl PlacedColumns {
    /// The left and right edges of the plot of a panel from column `first` to column `last`.
    fn edges(&self, first: usize, last: usize) -> (f64, f64) {
        (self.plot_x[first], self.plot_x[last] + self.plot_width)
    }

    /// The left and right edges of the plot of the panel covering `cells`.
    fn span(&self, cells: CellRange) -> (f64, f64) {
        self.edges(cells.col(), cells.col() + cells.col_span() - 1)
    }

    /// The left and right edges of the frame of the panel covering `cells` on a canvas
    /// `canvas` pt wide: the middle of the gap between its bands and the neighbouring column's,
    /// or the canvas edge.
    fn frame(&self, cells: CellRange, canvas: f64) -> (f64, f64) {
        let first = cells.col();
        let last = first + cells.col_span() - 1;
        // The middle of the gap between column `c` and column `c + 1`.
        let middle = |c: usize| {
            let end = self.plot_x[c] + self.plot_width + self.right[c];
            let start = self.plot_x[c + 1] - self.left[c + 1];
            (end + start) / 2.0
        };
        let left = if first == 0 { 0.0 } else { middle(first - 1) };
        let right = if last + 1 == self.plot_x.len() {
            canvas
        } else {
            middle(last)
        };
        (left, right)
    }
}

/// One bound an x tick label puts on the growth of one band: the label's end on one side must
/// stay within that side's limit. The band must grow by at least
/// `half - share × span - base`, where `span` is the width of the panel's plot.
#[derive(Debug, Clone, Copy, PartialEq)]
struct LabelBound {
    /// The panel, as an index into the grid's panels.
    panel: usize,
    /// The panel's first column.
    first: usize,
    /// The panel's last column.
    last: usize,
    /// The growth entry the bound is on (see [`Columns::no_growth`]).
    growth: usize,
    /// Half the label's width.
    half: f64,
    /// How far the label's tick lies from the plot edge on the bound's side, as a share of the
    /// plot width.
    share: f64,
    /// The room the label has beyond the plot edge before the band grows: the band itself
    /// and half the gap, or on a canvas edge the band and the padding less
    /// [`X_LABEL_EDGE_GAP`].
    base: f64,
}

impl LabelBound {
    /// How much the bound's band must grow for the label to stay within its limit on the
    /// `placed` columns; negative when it has room to spare.
    fn required(&self, placed: &PlacedColumns) -> f64 {
        let (left, right) = placed.edges(self.first, self.last);
        self.half - self.share * (right - left) - self.base
    }
}

impl Columns {
    /// The bands of the columns of `input`'s grid, from the panels covering `cells` with the
    /// texts measured in `bands`.
    fn measure(input: &GridInput<'_>, cells: &[CellRange], bands: &[PanelBands]) -> Columns {
        let cols = input.grid.cols();
        let mut left = vec![0.0_f64; cols];
        let mut right = vec![0.0_f64; cols];
        for (cells, bands) in cells.iter().zip(bands) {
            let first = cells.col();
            let last = first + cells.col_span() - 1;
            left[first] = left[first].max(bands.y_tick_column);
            right[last] = right[last].max(bands.right());
        }
        Columns {
            canvas: input.width,
            gap: input.gap(),
            left,
            right,
        }
    }

    /// The number of columns.
    fn count(&self) -> usize {
        self.left.len()
    }

    /// The growth vector of no growth. Its entries are the growth of the bands x tick labels
    /// may grow: entry 0 moves the grid in from the left canvas edge, entry 1 from the right
    /// one, and the others grow the band on one side of one column next to a gap (see
    /// [`Columns::left_growth`] and [`Columns::right_growth`]).
    fn no_growth(&self) -> Vec<f64> {
        vec![0.0; 2 * self.count()]
    }

    /// The growth entry that x tick labels reaching left of column `c`'s plots grow.
    fn left_growth(c: usize) -> usize {
        if c == 0 { 0 } else { 2 * c }
    }

    /// The growth entry that x tick labels reaching right of column `c`'s plots grow, of `cols`
    /// columns.
    fn right_growth(c: usize, cols: usize) -> usize {
        if c + 1 == cols { 1 } else { 2 * c + 3 }
    }

    /// The columns placed with their bands grown by `growth`.
    fn place(&self, growth: &[f64]) -> PlacedColumns {
        let cols = self.count();
        let left: Vec<f64> = (0..cols)
            .map(|c| {
                if c == 0 {
                    self.left[c]
                } else {
                    self.left[c] + growth[Columns::left_growth(c)]
                }
            })
            .collect();
        let right: Vec<f64> = (0..cols)
            .map(|c| {
                if c + 1 == cols {
                    self.right[c]
                } else {
                    self.right[c] + growth[Columns::right_growth(c, cols)]
                }
            })
            .collect();
        let inner_left = PADDING + growth[0];
        let inner_right = self.canvas - PADDING - growth[1];
        let bands = left.iter().sum::<f64>() + right.iter().sum::<f64>();
        let gaps = self.gap * (cols - 1) as f64;
        let plot_width = (inner_right - inner_left - bands - gaps) / cols as f64;
        let mut plot_x = Vec::with_capacity(cols);
        let mut x = inner_left + left[0];
        plot_x.push(x);
        for c in 1..cols {
            x += plot_width + right[c - 1] + self.gap + left[c];
            plot_x.push(x);
        }
        PlacedColumns {
            plot_x,
            plot_width,
            left,
            right,
        }
    }

    /// The bounds the x tick labels of the panels covering `cells`, measured in `bands`, put on
    /// the growth, with each label cut to at most `2 × half_max` wide.
    fn label_bounds(
        &self,
        cells: &[CellRange],
        bands: &[PanelBands],
        half_max: f64,
    ) -> Vec<LabelBound> {
        let cols = self.count();
        let edge_room = PADDING - X_LABEL_EDGE_GAP;
        let mut bounds = Vec::new();
        for (panel, (cells, bands)) in cells.iter().zip(bands).enumerate() {
            let first = cells.col();
            let last = first + cells.col_span() - 1;
            let left_room = if first == 0 {
                edge_room
            } else {
                self.gap / 2.0
            };
            let right_room = if last + 1 == cols {
                edge_room
            } else {
                self.gap / 2.0
            };
            for TickSpan { pos, half } in &bands.x_spans {
                let half = half.min(half_max);
                bounds.push(LabelBound {
                    panel,
                    first,
                    last,
                    growth: Columns::left_growth(first),
                    half,
                    share: *pos,
                    base: self.left[first] + left_room,
                });
                bounds.push(LabelBound {
                    panel,
                    first,
                    last,
                    growth: Columns::right_growth(last, cols),
                    half,
                    share: 1.0 - pos,
                    base: self.right[last] + right_room,
                });
            }
        }
        bounds
    }

    /// The bounds of `bounds` that can decide the growth: a bound is left out when another on
    /// the same band and the same panel columns requires at least as much at every plot width
    /// up to the canvas width (its requirement is linear in the width), the first of equal
    /// ones kept. Near-duplicate bounds, such as labels at positions 0 and 1e-10, would make
    /// the program's bases nearly singular.
    fn essential(&self, bounds: &[LabelBound]) -> Vec<LabelBound> {
        let at = |b: &LabelBound, span: f64| b.half - b.share * span - b.base;
        let covers = |j: &LabelBound, i: &LabelBound| {
            (j.growth, j.first, j.last) == (i.growth, i.first, i.last)
                && at(j, 0.0) >= at(i, 0.0)
                && at(j, self.canvas) >= at(i, self.canvas)
        };
        bounds
            .iter()
            .enumerate()
            .filter(|&(i, b)| {
                !bounds
                    .iter()
                    .enumerate()
                    .any(|(j, other)| j != i && covers(other, b) && (j < i || !covers(b, other)))
            })
            .map(|(_, b)| *b)
            .collect()
    }

    /// The linear program of the growth that fits `bounds` (see [`fit_columns`]), whose
    /// requirements on the `open` columns, without growth, are `at_open`: one row per bound,
    /// then the canvas edge caps and the smallest plot width.
    fn program(
        &self,
        bounds: &[LabelBound],
        at_open: &[f64],
        open: &PlacedColumns,
    ) -> Vec<program::Row> {
        let cols = self.count();
        let vars = 2 * cols;
        let mut rows: Vec<program::Row> = bounds
            .iter()
            .zip(at_open)
            .map(|(b, required)| {
                // growth[b] - narrowing × total + share × inner growth ≥ requirement.
                let narrowing = b.share * (b.last - b.first + 1) as f64 / cols as f64;
                let mut coefficients = vec![-narrowing; vars];
                coefficients[b.growth] += 1.0;
                for c in b.first..b.last {
                    coefficients[Columns::right_growth(c, cols)] += b.share;
                    coefficients[Columns::left_growth(c + 1)] += b.share;
                }
                program::Row {
                    coefficients,
                    bound: *required,
                }
            })
            .collect();
        let (cap_left, cap_right) = self.edge_caps();
        for (entry, cap) in [(0, cap_left), (1, cap_right)] {
            let mut coefficients = vec![0.0; vars];
            coefficients[entry] = -1.0;
            rows.push(program::Row {
                coefficients,
                bound: -cap,
            });
        }
        rows.push(program::Row {
            coefficients: vec![-1.0; vars],
            bound: -(open.plot_width - MIN_PLOT) * cols as f64,
        });
        rows
    }

    /// The columns placed for `growth`, a solution of [`Columns::program`] for `bounds`, once
    /// it is checked: `None` unless every label is within its bound to
    /// [`CONTAINMENT_TOLERANCE`], the distance by which a label may miss its bound and still
    /// count as contained, the canvas edges within their caps and the plots at least
    /// [`MIN_PLOT`] wide. A label out by more than that grows its band to its requirement on
    /// the placed columns, at most [`X_LABEL_PASSES`] times, which settles rounding beyond the
    /// tolerance.
    fn settle(&self, bounds: &[LabelBound], mut growth: Vec<f64>) -> Option<PlacedColumns> {
        let (cap_left, cap_right) = self.edge_caps();
        for pass in 0..=X_LABEL_PASSES {
            let placed = self.place(&growth);
            let within_caps = growth[0] <= cap_left + CONTAINMENT_TOLERANCE
                && growth[1] <= cap_right + CONTAINMENT_TOLERANCE
                && placed.plot_width >= MIN_PLOT - CONTAINMENT_TOLERANCE;
            if !within_caps {
                return None;
            }
            let required: Vec<f64> = bounds.iter().map(|b| b.required(&placed)).collect();
            let worst = bounds
                .iter()
                .zip(&required)
                .map(|(b, r)| r - growth[b.growth])
                .fold(0.0, f64::max);
            if worst <= CONTAINMENT_TOLERANCE {
                return Some(placed);
            }
            if pass == X_LABEL_PASSES {
                return None;
            }
            for (b, r) in bounds.iter().zip(&required) {
                growth[b.growth] = growth[b.growth].max(*r);
            }
        }
        None
    }

    /// The most the grid may move in from each canvas edge: as on a single panel, x tick labels
    /// may move a plot edge in to at most [`END_LABEL_SHARE`] of the inner width from the
    /// padding, and not at all where the band there is already wider.
    fn edge_caps(&self) -> (f64, f64) {
        let cap = end_label_cap(self.canvas);
        let last = self.count() - 1;
        (
            (cap - self.left[0]).max(0.0),
            (cap - self.right[last]).max(0.0),
        )
    }
}

/// Why the x tick label solve gave up: the panels whose labels were out of bounds when it did,
/// as indices into the grid's panels, in panel order.
#[derive(Debug, Clone, PartialEq)]
struct Stuck {
    panels: Vec<usize>,
}

/// The columns placed with their bands grown just enough that every x tick label of `bounds`
/// stays within its bound, or why no growth does.
///
/// Every bound's requirement is affine in the growth: growing any band by `g` narrows every
/// column's plot by `g / cols`, so a panel spanning `n` columns narrows by `n × g / cols`, but a
/// band between the panel's own columns also widens it by `g`. So the least total growth that
/// meets every bound, keeps the canvas edges within their caps (see [`Columns::edge_caps`])
/// and leaves the plots at least [`MIN_PLOT`] wide is a linear program (see
/// [`Columns::program`] and [`program::least_total`]). Nothing is solved when every label
/// already fits.
///
/// The program is solved first with every bound raised by [`PROGRAM_MARGIN`], so that the
/// rounding of the solve stays inside the margin and the growth meets the original bounds
/// with room to spare. Only when that program has no solution (a fit that is tight to within
/// the margin) is the program solved as it is, and when the rounding of its inputs leaves an
/// exactly tight fit without a solution there, again with every bound lowered by half of
/// [`CONTAINMENT_TOLERANCE`], the distance by which a label may miss its bound and still count
/// as contained. Either growth is checked on the placed columns before it is used
/// ([`Columns::settle`]): every label within its bound to [`CONTAINMENT_TOLERANCE`], the edges
/// within their caps, the plots at least [`MIN_PLOT`] wide. So the labels are refused only
/// when the lowered program has no solution: when no growth brings every label within the
/// containment tolerance of its bound.
fn fit_columns(
    columns: &Columns,
    bounds: &[LabelBound],
) -> std::result::Result<PlacedColumns, Stuck> {
    let open = columns.place(&columns.no_growth());
    let at_open: Vec<f64> = bounds.iter().map(|b| b.required(&open)).collect();
    let stuck = Stuck {
        panels: out_of_bounds(bounds, &at_open, &columns.no_growth(), MOVE_TOLERANCE),
    };
    if stuck.panels.is_empty() {
        return Ok(open);
    }
    let essential = columns.essential(bounds);
    let at_essential: Vec<f64> = essential.iter().map(|b| b.required(&open)).collect();
    let rows = columns.program(&essential, &at_essential, &open);
    let moved = |by: f64| -> Vec<program::Row> {
        rows.iter()
            .map(|row| program::Row {
                bound: row.bound + by,
                ..row.clone()
            })
            .collect()
    };
    let vars = 2 * columns.count();
    for program in [
        moved(PROGRAM_MARGIN),
        moved(0.0),
        moved(-CONTAINMENT_TOLERANCE / 2.0),
    ] {
        if let Some(growth) = program::least_total(vars, &program)
            && let Some(placed) = columns.settle(bounds, growth)
        {
            return Ok(placed);
        }
    }
    Err(stuck)
}

/// The panels with a bound of `bounds` whose requirement, `required`, exceeds its growth by
/// more than `tolerance`, each once, in panel order.
fn out_of_bounds(
    bounds: &[LabelBound],
    required: &[f64],
    growth: &[f64],
    tolerance: f64,
) -> Vec<usize> {
    let mut panels: Vec<usize> = bounds
        .iter()
        .zip(required)
        .filter(|(b, r)| **r - growth[b.growth] > tolerance)
        .map(|(b, _)| b.panel)
        .collect();
    panels.sort_unstable();
    panels.dedup();
    panels
}

/// The row bands of a grid, measured for the plot widths the columns gave the panels.
#[derive(Debug, Clone)]
struct Rows {
    /// The gap between neighbouring rows.
    gap: f64,
    /// `T[r]`: the band above each row's plots.
    above: Vec<f64>,
    /// `B[r]`: the band below each row's plots.
    below: Vec<f64>,
    /// The part of each bottom band occupied by legends, after the aligned x axis labels.
    legends: Vec<f64>,
}

/// The rows placed.
#[derive(Debug, Clone)]
struct PlacedRows {
    /// The top of each row: where its band above the plots starts.
    row_top: Vec<f64>,
    /// Each row's plot top edge.
    plot_y: Vec<f64>,
    /// The plot height of every row, `Q`.
    plot_height: f64,
    /// Each row's band below the plots.
    below: Vec<f64>,
    /// The part of each bottom band occupied by legends, after the aligned x axis labels.
    legends: Vec<f64>,
}

impl Rows {
    /// The bands of the rows of `input`'s grid, from the panels covering `cells` with the texts
    /// measured in `bands` and their plots running from left to right as in `spans`, which
    /// their legend rows wrap within.
    fn measure(
        input: &GridInput<'_>,
        cells: &[CellRange],
        bands: &[PanelBands],
        spans: &[(f64, f64)],
    ) -> Rows {
        let rows = input.grid.rows();
        let mut above = vec![0.0_f64; rows];
        let mut below = vec![0.0_f64; rows];
        let mut legends = vec![0.0_f64; rows];
        for (((cells, bands), panel), &(left, right)) in
            cells.iter().zip(bands).zip(input.panels).zip(spans)
        {
            let first = cells.row();
            let last = first + cells.row_span() - 1;
            let row = panel.legend.header_within(left, right);
            above[first] = above[first].max(bands.above_with_row(&row));
            below[last] = below[last].max(bands.below() - bands.bottom_legend);
            legends[last] = legends[last].max(panel.legend.bottom_reserve(left, right));
        }
        for (below, legend) in below.iter_mut().zip(&legends) {
            *below += legend;
        }
        Rows {
            gap: input.gap(),
            above,
            below,
            legends,
        }
    }

    /// The rows placed on `input`'s canvas.
    ///
    /// # Errors
    ///
    /// `Error::Style` in `input.context` when the plots of the rows are shorter than
    /// [`MIN_PLOT`].
    fn place(self, input: &GridInput<'_>) -> Result<PlacedRows> {
        let rows = self.above.len();
        let bands = self.above.iter().sum::<f64>() + self.below.iter().sum::<f64>();
        let gaps = self.gap * (rows - 1) as f64;
        let padding = input.top + PADDING;
        let plot_height = |height: f64| (height - padding - bands - gaps) / rows as f64;
        if plot_height(input.height) < MIN_PLOT {
            // The least whole height that the check accepts.
            let needed = padding + bands + gaps + MIN_PLOT * rows as f64;
            let least = input.height.floor() + 1.0;
            let mut whole = needed.ceil().max(least);
            while plot_height(whole) < MIN_PLOT {
                whole += 1.0;
            }
            while whole > least && plot_height(whole - 1.0) >= MIN_PLOT {
                whole -= 1.0;
            }
            return Err(grid_too_short(input.context, input.height, whole, rows));
        }
        let plot_height = plot_height(input.height);
        let mut row_top = Vec::with_capacity(rows);
        let mut plot_y = Vec::with_capacity(rows);
        let mut y = input.top;
        for r in 0..rows {
            row_top.push(y);
            let top = y + self.above[r];
            plot_y.push(top);
            y = top + plot_height + self.below[r] + self.gap;
        }
        Ok(PlacedRows {
            row_top,
            plot_y,
            plot_height,
            below: self.below,
            legends: self.legends,
        })
    }
}

impl PlacedRows {
    /// The top and bottom edges of the plot of the panel covering `cells`.
    fn span(&self, cells: CellRange) -> (f64, f64) {
        let last = cells.row() + cells.row_span() - 1;
        (
            self.plot_y[cells.row()],
            self.plot_y[last] + self.plot_height,
        )
    }

    /// The top and bottom edges of the frame of the panel covering `cells` on a canvas `canvas`
    /// pt tall: the middle of the gap between its bands and the neighbouring row's, or the
    /// canvas edge.
    fn frame(&self, cells: CellRange, canvas: f64) -> (f64, f64) {
        let first = cells.row();
        let last = first + cells.row_span() - 1;
        // The middle of the gap between row `r` and row `r + 1`.
        let middle = |r: usize| {
            let end = self.plot_y[r] + self.plot_height + self.below[r];
            (end + self.row_top[r + 1]) / 2.0
        };
        let top = if first == 0 { 0.0 } else { middle(first - 1) };
        let bottom = if last + 1 == self.plot_y.len() {
            canvas
        } else {
            middle(last)
        };
        (top, bottom)
    }
}

/// The error for columns whose plots are narrower than [`MIN_PLOT`] before x tick labels are
/// fitted.
///
/// The plots lack `deficit` pt in all. If the y tick labels of one column, narrower by the
/// deficit, would leave room, they are the cause: the error names the widest y tick label of
/// the widest such column, in the first panel whose labels are that wide (see
/// [`y_labels_leave_no_room`]). Otherwise it names the width the figure needs (see
/// [`grid_too_narrow`]).
fn columns_too_narrow(
    input: &GridInput<'_>,
    cells: &[CellRange],
    bands: &[PanelBands],
    columns: &Columns,
) -> Error {
    let cols = columns.count();
    let placed = columns.place(&columns.no_growth());
    let deficit = (MIN_PLOT - placed.plot_width) * cols as f64;
    let culprit = cells
        .iter()
        .zip(bands)
        .filter(|(_, b)| !b.y_labels.is_empty() && b.y_label_width - deficit >= 0.0)
        .filter(|(c, b)| b.y_tick_column == columns.left[c.col()])
        .fold(
            None,
            |best: Option<(&CellRange, &PanelBands)>, (c, b)| match best {
                Some((_, kept)) if kept.y_tick_column >= b.y_tick_column => best,
                _ => Some((c, b)),
            },
        );
    if let Some((cells, bands)) = culprit
        && let Some(label) = widest(&bands.y_labels)
    {
        let err = y_labels_leave_no_room(input.context, label, bands.y_label_width - deficit);
        return in_panel(*cells, err);
    }
    grid_too_narrow(
        input.context,
        input.width,
        needed_width(input, cells, columns),
        cols,
    )
}

/// The narrowest whole-point canvas width, wider than the figure's, on which the plots of the
/// panels covering `cells`, with the y tick label columns of `columns`, are at least
/// [`MIN_PLOT`] wide before x tick labels are fitted.
///
/// The plots' width is `(W - 2 × PADDING - ΣL - ΣR(W) - gap × (cols - 1)) / cols`, where
/// `ΣR(W)`, the room direct labels reserve, depends on the width `W`: each panel's labels
/// reserve their own width but at most a share of the panel's cells, which are 0 wide up to
/// `2 × PADDING + gap × (cols - 1)`. Between that width and the widths where one panel's cap
/// reaches another's labels in the same column (where `ΣR` changes slope), the plot width is
/// linear in `W`, so the narrowest width is found segment by segment from the figure's width,
/// and then checked both ways: the check accepts the result and refuses one point less.
fn needed_width(input: &GridInput<'_>, cells: &[CellRange], columns: &Columns) -> f64 {
    let cols = columns.count();
    let gap = columns.gap;
    // The columns of a canvas `width` pt wide: the same y tick label columns, and the direct
    // label reserves capped for that width.
    let at = |width: f64| -> Columns {
        let wider = GridInput { width, ..*input };
        let mut right = vec![0.0_f64; cols];
        for (cells, panel) in cells.iter().zip(input.panels) {
            let last = cells.col() + cells.col_span() - 1;
            let cap = wider.span(*cells).reserve_cap;
            right[last] = right[last].max(
                panel
                    .legend
                    .direct
                    .reserve(cap)
                    .max(panel.legend.right_reserve()),
            );
        }
        Columns {
            canvas: width,
            right,
            ..columns.clone()
        }
    };
    let room = |width: f64| at(width).place(&at(width).no_growth()).plot_width - MIN_PLOT;
    // The widths where a panel's cap equals the uncapped reserve of a panel ending in the same
    // column: cap = END_LABEL_SHARE × (cell × span + gap × (span - 1)), with
    // cell = (W - 2 × PADDING - gap × (cols - 1)) / cols.
    let wants: Vec<(usize, usize, f64)> = cells
        .iter()
        .zip(input.panels)
        .map(|(cells, panel)| {
            let last = cells.col() + cells.col_span() - 1;
            (
                last,
                cells.col_span(),
                panel
                    .legend
                    .direct
                    .reserve(f64::INFINITY)
                    .max(panel.legend.right_reserve()),
            )
        })
        .collect();
    // The width below which the cells, and so the caps, stop shrinking: they are 0 wide.
    let mut points: Vec<f64> = vec![2.0 * PADDING + gap * (cols - 1) as f64];
    for &(column, span, _) in &wants {
        for &(_, _, want) in wants.iter().filter(|w| w.0 == column) {
            let cell = (want / END_LABEL_SHARE - gap * (span - 1) as f64) / span as f64;
            points.push(cell * cols as f64 + 2.0 * PADDING + gap * (cols - 1) as f64);
        }
    }
    // Past every such width each reserve is its own; the plots are then wide enough at the sum
    // of every band at its widest.
    let widest_reserves: f64 = (0..cols)
        .map(|c| {
            wants
                .iter()
                .filter(|w| w.0 == c)
                .map(|w| w.2)
                .fold(0.0, f64::max)
        })
        .sum();
    let last = columns.left.iter().sum::<f64>()
        + widest_reserves
        + gap * (cols - 1) as f64
        + MIN_PLOT * cols as f64
        + 2.0 * PADDING;
    points.push(last);
    let start = input.width;
    points.retain(|p| p.is_finite() && *p > start);
    points.sort_by(f64::total_cmp);
    points.push(points.last().map_or(last, |p| p.max(last)) + 1.0);
    // The first segment on which the room becomes nonnegative, solved linearly.
    let (mut low, mut at_low) = (start, room(start));
    let mut width = last;
    for &high in &points {
        let at_high = room(high);
        if at_high >= 0.0 {
            width = if at_high > at_low {
                low + (high - low) * (-at_low / (at_high - at_low))
            } else {
                high
            };
            break;
        }
        (low, at_low) = (high, at_high);
    }
    // A whole width the check accepts, one point narrower than which it does not: rounding,
    // or a segment on which the room falls again, can move it by a few points either way.
    let least = input.width.floor() + 1.0;
    let mut whole = width.ceil().max(least);
    while room(whole) < 0.0 && whole < last.ceil() + 1.0 {
        whole += 1.0;
    }
    while whole > least && room(whole - 1.0) >= 0.0 {
        whole -= 1.0;
    }
    whole
}

/// The error for x tick labels that no growth of the bands fits (see [`fit_columns`]): it names
/// the widest x tick label among the panels in `stuck` (the first of equally wide ones), in its
/// panel, and the width up to which every x tick label of the figure would fit.
fn x_labels_do_not_fit(
    input: &GridInput<'_>,
    cells: &[CellRange],
    bands: &[PanelBands],
    columns: &Columns,
    stuck: &Stuck,
) -> Error {
    let named = stuck
        .panels
        .iter()
        .copied()
        .fold(None, |best: Option<usize>, i| {
            let width = |i: usize| widest(&bands[i].x_labels).map_or(0.0, Label::width);
            match best {
                Some(kept) if width(kept) >= width(i) => best,
                _ => Some(i),
            }
        })
        .unwrap_or(0);
    let fits = widest_label_that_fits(columns, cells, bands);
    let err = x_labels_wider_than(input.context, &bands[named].x_labels, fits);
    in_panel(cells[named], err)
}

/// The largest width `w` such that the x tick labels of every panel, each cut to at most `w`
/// wide (and kept centered on its tick), fit the grid: [`fit_columns`] places them. It is 0
/// when not even labels of no width fit, and at most the widest label's width. Cutting labels
/// only relaxes the bounds, so bisection finds it.
fn widest_label_that_fits(columns: &Columns, cells: &[CellRange], bands: &[PanelBands]) -> f64 {
    let fits =
        |half_max: f64| fit_columns(columns, &columns.label_bounds(cells, bands, half_max)).is_ok();
    let widest = bands
        .iter()
        .flat_map(|b| &b.x_spans)
        .map(|s| s.half)
        .fold(0.0, f64::max);
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

/// `count` followed by `one` or by `many`, as English counts it.
fn counted(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// The error for a canvas `width` pt wide whose `cols` columns of plots need at least `needed`
/// pt. The widths are in whole points, as for a single panel: the figure's rounded down, the
/// one needed rounded up and at least one point more.
fn grid_too_narrow(context: &'static str, width: f64, needed: f64, cols: usize) -> Error {
    let shown = width.floor();
    Error::style(
        context,
        format!(
            "the figure is {} pt wide; at least {} pt is needed for {} of plots.",
            number(shown),
            number(needed.ceil().max(shown + 1.0)),
            counted(cols, "column", "columns"),
        ),
    )
    .with_hint(if cols == 1 {
        "widen the figure."
    } else {
        "widen the figure, or use fewer columns."
    })
}

/// The error for a canvas `height` pt tall whose `rows` rows of plots need at least `needed`
/// pt, in whole points as [`grid_too_narrow`].
fn grid_too_short(context: &'static str, height: f64, needed: f64, rows: usize) -> Error {
    let shown = height.floor();
    Error::style(
        context,
        format!(
            "the figure is {} pt tall; at least {} pt is needed for {} of plots.",
            number(shown),
            number(needed.ceil().max(shown + 1.0)),
            counted(rows, "row", "rows"),
        ),
    )
    .with_hint(if rows == 1 {
        "make the figure taller, or remove titles or legends."
    } else {
        "make the figure taller, use fewer rows, or remove titles or legends."
    })
}

/// Checks that the texts of every panel of `input`, placed as in `layouts` (in the same
/// order), stay in the panel's cells, on a grid of more than one cell; a figure of one cell
/// is not checked. Text that does not would run into a neighbouring cell. The final layout is
/// checked, after the second pass of the layout, whose plots can be wider than the first's.
///
/// In panel order, then in this order: the title, the subtitle and every legend row entry must
/// end within the plot width from the plot's left edge; the x axis label and the y axis label
/// must be at most as wide as the plot; every direct label must end within the panel's frame.
///
/// # Errors
///
/// `Error::Style` in `input.context` for the first text that does not, with the widths in
/// whole points: the room rounded down, the text's width rounded and at least one point more.
/// - `row {r}, col {c}: the title is {w} pt wide; the panel has {p} pt.` (likewise
///   `the subtitle`, `the x axis label` and `the y axis label`), hint
///   `shorten the title or widen the figure.` (naming the text);
/// - `row {r}, col {c}: the legend label "…" is {w} pt wide; the panel has {p} pt.`, with `p`
///   the room from the label's start to the plot's right edge;
/// - `row {r}, col {c}: the direct label "…" is {w} pt wide; at most {m} pt fits beside the
///   plot.`, with `m` the room from the label's start to the frame's right edge.
///
/// Both legend errors have the hint `shorten the label or widen the figure.`
pub(crate) fn check_panel_contents(input: &GridInput<'_>, layouts: &[Layout]) -> Result<()> {
    if input.is_single() {
        for (panel, layout) in input.panels.iter().zip(layouts) {
            super::placement::check(panel, layout, true)?;
        }
        return Ok(());
    }
    let too_wide = |cells: CellRange, message: String, hint: &'static str| {
        in_panel(cells, Error::style(input.context, message).with_hint(hint))
    };
    for (panel, layout) in input.panels.iter().zip(layouts) {
        let cells = panel.axes.cells();
        let plot = layout.plot;
        let named: [(Option<&Label>, &str, &'static str); 2] = [
            (
                layout.title.as_ref(),
                "the title",
                "shorten the title or widen the figure.",
            ),
            (
                layout.subtitle.as_ref(),
                "the subtitle",
                "shorten the subtitle or widen the figure.",
            ),
        ];
        let axis: [(Option<&Label>, &str, &'static str); 2] = [
            (
                layout.xlabel.as_ref(),
                "the x axis label",
                "shorten the x axis label or widen the figure.",
            ),
            (
                layout.ylabel.as_ref(),
                "the y axis label",
                "shorten the y axis label or widen the figure.",
            ),
        ];
        for (label, name, hint) in named {
            if let Some(label) = label
                && label.width() > plot.width + CONTAINMENT_TOLERANCE
            {
                let (w, p) = widths(label.width(), plot.width);
                let message = format!("{name} is {w} pt wide; the panel has {p} pt.");
                return Err(too_wide(cells, message, hint));
            }
        }
        super::placement::check(panel, layout, false).map_err(|err| in_panel(cells, err))?;
        for (label, name, hint) in axis {
            if let Some(label) = label
                && label.width() > plot.width + CONTAINMENT_TOLERANCE
            {
                let (w, p) = widths(label.width(), plot.width);
                let message = format!("{name} is {w} pt wide; the panel has {p} pt.");
                return Err(too_wide(cells, message, hint));
            }
        }
        for label in layout.direct.iter().filter_map(|d| d.label.as_ref()) {
            if label.x + label.width() > layout.frame.right() + CONTAINMENT_TOLERANCE {
                let (w, m) = widths(label.width(), layout.frame.right() - label.x);
                let message = format!(
                    "the direct label {} is {w} pt wide; at most {m} pt fits beside the plot.",
                    quoted(&label.text)
                );
                return Err(too_wide(cells, message, LEGEND_HINT));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Layout;
    use crate::model::{AxesMut, AxisData, BarOptions, Figure, GridIndex, LineOptions, Placement};
    use crate::pipeline::try_plan_panels;
    use crate::{FontLibrary, LegendStyle, ScaleKind};
    use proptest::prelude::*;

    fn lib() -> &'static FontLibrary {
        FontLibrary::global()
    }

    fn themes() -> [Theme; 3] {
        [Theme::pigment(), Theme::graphite(), Theme::midnight()]
    }

    /// An empty `rows × cols` figure, `width` by `height` in.
    fn grid_figure(rows: i64, cols: i64, (width, height): (f64, f64), theme: Theme) -> Figure {
        Figure::with_grid(width, height, theme, Grid::new(rows, cols).unwrap()).unwrap()
    }

    /// The axes added at `row`, `col`, spanning `row_span × col_span` cells, on linear scales.
    fn add(f: &mut Figure, row: i64, col: i64, row_span: i64, col_span: i64) -> AxesMut<'_> {
        let at = Placement {
            row: GridIndex::At(row),
            col: GridIndex::At(col),
            row_span,
            col_span,
        };
        f.add_axes_at(at, ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
    }

    /// A line from (0, 0) to (10, 1).
    fn line(axes: &mut AxesMut<'_>) {
        axes.line(
            AxisData::Numbers(vec![0.0, 10.0]),
            &[0.0, 1.0],
            LineOptions::default(),
        )
        .unwrap();
    }

    /// Every axes' cells and layout, in panel order.
    fn layouts(f: &Figure) -> Result<Vec<(CellRange, Layout)>> {
        let planned = try_plan_panels(f, lib())?;
        Ok(f.panels()
            .map(Axes::cells)
            .zip(planned.into_iter().map(|(_, layout)| layout))
            .collect())
    }

    use crate::model::Axes;

    fn render_error(f: &Figure) -> Error {
        f.to_scene(lib()).expect_err("the figure does not lay out")
    }

    /// A small deterministic generator for the random grids.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            // xorshift64*
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
        }

        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }

        /// A value in `0.0..1.0`.
        fn unit(&mut self) -> f64 {
            (self.next() >> 11) as f64 / (1_u64 << 53) as f64
        }

        fn text(&mut self, max: usize) -> String {
            let words = [
                "Load", "North", "Station", "Mean", "W", "Day", "output", "%",
            ];
            let mut out = String::new();
            for _ in 0..self.below(max + 1) {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(words[self.below(words.len())]);
            }
            out
        }
    }

    /// A `rows × cols` grid at its automatic size in `theme`, filled from `seed`: cells left
    /// empty or holding axes with random spans, texts, legends and tick labels.
    fn random_grid(
        rows: usize,
        cols: usize,
        seed: u64,
        theme: Theme,
        sharing: [crate::Share; 2],
    ) -> Figure {
        use crate::Share;
        let mut rng = Rng(seed | 1);
        let grid = Grid::new(rows as i64, cols as i64)
            .unwrap()
            .with_share_x(sharing[0])
            .with_share_y(sharing[1]);
        let (width, height) = grid.auto_size_in();
        let mut f = Figure::with_grid(width, height, theme, grid).unwrap();
        let mut taken = vec![vec![false; cols]; rows];
        for row in 0..rows {
            for col in 0..cols {
                if taken[row][col] || rng.below(5) == 0 {
                    continue;
                }
                let mut col_span = if sharing.contains(&Share::Col) {
                    1
                } else {
                    1 + rng.below(3.min(cols - col))
                };
                while (col..col + col_span).any(|c| taken[row][c]) {
                    col_span -= 1;
                }
                let mut row_span = if sharing.contains(&Share::Row) {
                    1
                } else {
                    1 + rng.below(3.min(rows - row))
                };
                while (row..row + row_span).any(|r| (col..col + col_span).any(|c| taken[r][c])) {
                    row_span -= 1;
                }
                for cells in taken.iter_mut().skip(row).take(row_span) {
                    for cell in cells.iter_mut().skip(col).take(col_span) {
                        *cell = true;
                    }
                }
                let category = sharing[0] == Share::None && rng.below(3) == 0;
                let at = Placement {
                    row: GridIndex::At(row as i64),
                    col: GridIndex::At(col as i64),
                    row_span: row_span as i64,
                    col_span: col_span as i64,
                };
                let x = if category {
                    ScaleKind::Category
                } else {
                    ScaleKind::Linear
                };
                let mut axes = f.add_axes_at(at, x, ScaleKind::Linear).unwrap();
                axes.title(&rng.text(3))
                    .subtitle(&rng.text(2))
                    .xlabel(&rng.text(2))
                    .ylabel(&rng.text(1));
                if rng.below(2) == 0 && axes.y_ticks().is_none() {
                    let labels: Vec<String> = (0..3).map(|_| rng.text(2)).collect();
                    axes.yticks_labeled(&[0.0, 5.0, 10.0], labels).unwrap();
                }
                let style = [
                    LegendStyle::Header,
                    LegendStyle::Chips,
                    LegendStyle::Direct,
                    LegendStyle::None,
                ][rng.below(4)];
                axes.legend(style);
                if category {
                    let names: Vec<String> = (0..1 + rng.below(4))
                        .map(|i| format!("{i}{}", rng.text(1)))
                        .collect();
                    let heights: Vec<f64> = (0..names.len()).map(|i| i as f64 + 1.0).collect();
                    axes.bar(
                        names,
                        &heights,
                        BarOptions {
                            label: rng.text(1),
                            ..BarOptions::default()
                        },
                    )
                    .unwrap();
                } else {
                    for _ in 0..1 + rng.below(3) {
                        let end = 1.0 + rng.below(1000) as f64;
                        axes.line(
                            AxisData::Numbers(vec![0.0, end / 3.0, end]),
                            &[0.0, rng.below(10) as f64, 10.0],
                            LineOptions {
                                label: rng.text(2),
                                ..LineOptions::default()
                            },
                        )
                        .unwrap();
                    }
                }
            }
        }
        f
    }

    /// Asserts that equal values are equal within 1e-9 pt.
    fn same(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-9 * (1.0 + a.abs().max(b.abs()))
    }

    /// Asserts the alignment rules on `layouts`: equal single-cell plots, plot edges shared by
    /// every panel starting or ending in a grid column or row.
    fn assert_aligned(layouts: &[(CellRange, Layout)]) -> std::result::Result<(), TestCaseError> {
        let single: Vec<&Layout> = layouts
            .iter()
            .filter(|(c, _)| c.row_span() == 1 && c.col_span() == 1)
            .map(|(_, l)| l)
            .collect();
        if let Some(first) = single.first() {
            for l in &single {
                prop_assert!(same(l.plot.width, first.plot.width), "{:?}", l.plot);
                prop_assert!(same(l.plot.height, first.plot.height), "{:?}", l.plot);
            }
        }
        for (a_cells, a) in layouts {
            for v in [a.plot.x, a.plot.y, a.plot.width, a.plot.height] {
                prop_assert!(v.is_finite(), "{:?}", a.plot);
            }
            prop_assert!(a.plot.width >= MIN_PLOT, "{:?}", a.plot);
            prop_assert!(a.plot.height >= MIN_PLOT, "{:?}", a.plot);
            for (b_cells, b) in layouts {
                let (a_last_col, b_last_col) = (
                    a_cells.col() + a_cells.col_span(),
                    b_cells.col() + b_cells.col_span(),
                );
                let (a_last_row, b_last_row) = (
                    a_cells.row() + a_cells.row_span(),
                    b_cells.row() + b_cells.row_span(),
                );
                if a_cells.col() == b_cells.col() {
                    prop_assert!(same(a.plot.x, b.plot.x));
                }
                if a_last_col == b_last_col {
                    prop_assert!(same(a.plot.right(), b.plot.right()));
                }
                if a_cells.row() == b_cells.row() {
                    prop_assert!(same(a.plot.y, b.plot.y));
                }
                if a_last_row == b_last_row {
                    prop_assert!(same(a.plot.bottom(), b.plot.bottom()));
                }
                // A panel left of another in the same rows ends before it starts; likewise
                // down. Frames never overlap.
                if a_last_col <= b_cells.col() {
                    prop_assert!(a.plot.right() < b.plot.x);
                    prop_assert!(a.frame.right() <= b.frame.x + 1e-9);
                }
                if a_last_row <= b_cells.row() {
                    prop_assert!(a.plot.bottom() < b.plot.y);
                    prop_assert!(a.frame.bottom() <= b.frame.y + 1e-9);
                }
            }
        }
        Ok(())
    }

    proptest! {
        // Apply the environment after these defaults so stress runs can override them.
        #![proptest_config(proptest::test_runner::contextualize_config(ProptestConfig {
            cases: 256,
            rng_seed: proptest::test_runner::RngSeed::Fixed(38572056803),
            max_global_rejects: 1_024,
            ..ProptestConfig::default()
        }))]

        #[test]
        fn plot_areas_are_equal_and_aligned(
            rows in 1_usize..=5,
            cols in 1_usize..=5,
            seed in any::<u64>(),
            theme in 0_usize..3,
            share_x in 0_usize..4,
            share_y in 0_usize..4,
        ) {
            let theme = themes()[theme].clone();
            let modes = [crate::Share::None, crate::Share::Row, crate::Share::Col, crate::Share::All];
            let f = random_grid(rows, cols, seed, theme, [modes[share_x], modes[share_y]]);
            match layouts(&f) {
                Ok(layouts) => assert_aligned(&layouts)?,
                // Random texts may not fit their panels; such a grid fails naming one.
                Err(err) => {
                    let message = err.to_string();
                    prop_assert!(matches!(err, Error::Style { .. }), "{message}");
                    prop_assert!(message.contains(" pt wide; the panel has "), "{message}");
                }
            }
        }
    }

    #[test]
    fn sparse_grids_keep_equal_cells() {
        // Without tick labels, titles or axis labels every band is 0, so the plots are the
        // inner size less the gaps, split evenly.
        let bare = |axes: &mut AxesMut<'_>| {
            axes.xticks(&[]).unwrap().yticks(&[]).unwrap();
            line(axes);
        };
        let gap = 24.0;
        let (w, h) = (9.0 * 72.0, 6.0 * 72.0);
        let share = |extent: f64, n: f64| (extent - 2.0 * PADDING - gap * (n - 1.0)) / n;
        for theme in themes() {
            // 3 × 3 holding one 2 × 2 axes at (0, 0) and nothing else.
            let mut f = grid_figure(3, 3, (9.0, 6.0), theme.clone());
            bare(&mut add(&mut f, 0, 0, 2, 2));
            let all = layouts(&f).unwrap();
            let plot = all[0].1.plot;
            let (p, q) = (share(w, 3.0), share(h, 3.0));
            assert!(same(plot.x, PADDING) && same(plot.y, PADDING), "{plot:?}");
            assert!(same(plot.width, 2.0 * p + gap), "{plot:?}");
            assert!(same(plot.height, 2.0 * q + gap), "{plot:?}");

            // 2 × 3 with column 1 empty.
            let mut f = grid_figure(2, 3, (9.0, 6.0), theme.clone());
            for (row, col) in [(0, 0), (0, 2), (1, 0), (1, 2)] {
                bare(&mut add(&mut f, row, col, 1, 1));
            }
            let (p, q) = (share(w, 3.0), share(h, 2.0));
            for (cells, layout) in layouts(&f).unwrap() {
                let plot = layout.plot;
                let x = PADDING + cells.col() as f64 * (p + gap);
                let y = PADDING + cells.row() as f64 * (q + gap);
                assert!(same(plot.width, p) && same(plot.height, q), "{plot:?}");
                assert!(same(plot.x, x) && same(plot.y, y), "{cells:?} {plot:?}");
            }

            // 3 × 2 with row 1 empty.
            let mut f = grid_figure(3, 2, (9.0, 6.0), theme.clone());
            for (row, col) in [(0, 0), (0, 1), (2, 0), (2, 1)] {
                bare(&mut add(&mut f, row, col, 1, 1));
            }
            let (p, q) = (share(w, 2.0), share(h, 3.0));
            for (cells, layout) in layouts(&f).unwrap() {
                let plot = layout.plot;
                let y = PADDING + cells.row() as f64 * (q + gap);
                assert!(same(plot.width, p) && same(plot.height, q), "{plot:?}");
                assert!(same(plot.y, y), "{cells:?} {plot:?}");
            }

            // The same grids with ordinary texts and tick labels: no NaN, aligned, and the
            // empty column and row keep a plot's share between their neighbours.
            let mut f = grid_figure(2, 3, (9.0, 6.0), theme.clone());
            for (row, col) in [(0, 0), (0, 2), (1, 0), (1, 2)] {
                let mut axes = add(&mut f, row, col, 1, 1);
                axes.title("Load").xlabel("Day").ylabel("MW");
                line(&mut axes);
            }
            let all = layouts(&f).unwrap();
            assert_aligned(&all).unwrap();
            let (left, right) = (&all[0].1, &all[1].1);
            let empty = right.plot.x - left.plot.right();
            assert!(empty > left.plot.width + 2.0 * gap - 1e-9, "{empty}");
        }
    }

    /// The width named by a grid error `the figure is … pt wide; at least {n} pt is needed …`.
    fn named_width(err: &Error) -> Option<f64> {
        let message = err.to_string();
        let (_, rest) = message.split_once(" pt wide; at least ")?;
        rest.split_once(" pt is needed for ")?.0.parse().ok()
    }

    /// The height named by a grid error `the figure is … pt tall; at least {n} pt is needed …`.
    fn named_height(err: &Error) -> Option<f64> {
        let message = err.to_string();
        let (_, rest) = message.split_once(" pt tall; at least ")?;
        rest.split_once(" pt is needed for ")?.0.parse().ok()
    }

    #[test]
    fn grid_room_errors_name_the_panel() {
        // Width: three columns without tick labels need the padding, two gaps and three plots
        // of MIN_PLOT: 24 + 48 + 3 = 75 pt.
        let columns = |width: f64| {
            let mut f = grid_figure(1, 3, (width / 72.0, 4.0), Theme::pigment());
            for col in 0..3 {
                let mut axes = add(&mut f, 0, col, 1, 1);
                axes.xticks(&[]).unwrap().yticks(&[]).unwrap();
                line(&mut axes);
            }
            f
        };
        let err = render_error(&columns(74.0));
        assert_eq!(
            err.to_string(),
            "Figure.to_scene(): the figure is 74 pt wide; at least 75 pt is needed for 3 \
             columns of plots."
        );
        assert_eq!(err.hint(), Some("widen the figure, or use fewer columns."));
        assert_eq!(named_width(&err), Some(75.0));
        assert!(columns(75.0).to_scene(lib()).is_ok());

        // y tick labels: the same columns with long y tick labels in column 2 need those labels
        // and their gap too; one point less fails naming the labels, one point more renders.
        let label = "W".repeat(12);
        let labelled = |width: f64| {
            let mut f = columns(width);
            f.axes_at_mut(GridIndex::At(0), GridIndex::At(2))
                .unwrap()
                .yticks_labeled(&[0.0, 1.0], vec!["0".into(), label.clone()])
                .unwrap();
            f
        };
        let probe = labelled(400.0);
        let (_, layout) = &layouts(&probe).unwrap()[2];
        let label_width = layout
            .y_tick_labels
            .iter()
            .map(super::super::Label::width)
            .fold(0.0, f64::max);
        let boundary = 75.0 + label_width + super::super::Y_LABEL_GAP;
        let failing = boundary.ceil() - 1.0;
        let err = render_error(&labelled(failing));
        let deficit = boundary - failing;
        let fits = (label_width - deficit).floor();
        assert_eq!(
            err.to_string(),
            format!(
                "Figure.to_scene(): row 0, col 2: y tick label \"{label}\" is {} pt wide; at \
                 most {fits} pt leaves room for the plot area.",
                label_width.round().max(fits + 1.0)
            )
        );
        assert_eq!(
            err.hint(),
            Some("shorten the y tick labels or widen the figure.")
        );
        assert!(labelled(failing + 1.0).to_scene(lib()).is_ok());

        // x tick labels: a wide last x tick label of the left panel of two reaches into the gap
        // up to its middle, 12 pt past the plot, and the left column's band grows by the rest.
        // Long y tick labels give the label room to reach left under them, so only the plot
        // width limits it: the plots are MIN_PLOT wide at 24 + 24 + L + (w / 2 - 12) + 2 pt,
        // with L the y tick label column.
        let wide = "W".repeat(8);
        let x_labelled = |width: f64| {
            let mut f = grid_figure(1, 2, (width / 72.0, 4.0), Theme::pigment());
            for col in 0..2 {
                let mut axes = add(&mut f, 0, col, 1, 1);
                axes.yticks(&[]).unwrap().xticks(&[]).unwrap();
                line(&mut axes);
            }
            f.axes_at_mut(GridIndex::At(0), GridIndex::At(0))
                .unwrap()
                .yticks_labeled(&[0.0, 1.0], vec![String::new(), "W".repeat(12)])
                .unwrap()
                .xticks_labeled(&[0.0, 10.0], vec![String::new(), wide.clone()])
                .unwrap();
            f
        };
        let (_, layout) = &layouts(&x_labelled(400.0)).unwrap()[0];
        let w = layout.x_tick_labels[1].width();
        let column = layout.plot.x - PADDING;
        let boundary = 38.0 + column + w / 2.0;
        let failing = boundary.ceil() - 1.0;
        let err = render_error(&x_labelled(failing));
        // At `failing` pt, labels up to 2 × (failing - 38 - L) pt wide fit.
        let fits = (2.0 * (failing - 38.0 - column)).floor();
        assert_eq!(
            err.to_string(),
            format!(
                "Figure.to_scene(): row 0, col 0: x tick label \"{wide}\" is {} pt wide; at most \
                 {fits} pt fits this figure.",
                w.round().max(fits + 1.0)
            )
        );
        assert_eq!(err.hint(), Some("shorten the labels or widen the figure."));
        assert!(x_labelled(failing + 1.0).to_scene(lib()).is_ok());

        // Height: two rows without texts need the padding, the gap and two plots: 24 + 24 + 2.
        let two_rows = |height: f64| {
            let mut f = grid_figure(2, 1, (4.0, height / 72.0), Theme::pigment());
            for row in 0..2 {
                let mut axes = add(&mut f, row, 0, 1, 1);
                axes.xticks(&[]).unwrap().yticks(&[]).unwrap();
                line(&mut axes);
            }
            f
        };
        let err = render_error(&two_rows(49.0));
        assert_eq!(
            err.to_string(),
            "Figure.to_scene(): the figure is 49 pt tall; at least 50 pt is needed for 2 rows \
             of plots."
        );
        assert_eq!(
            err.hint(),
            Some("make the figure taller, use fewer rows, or remove titles or legends.")
        );
        assert_eq!(named_height(&err), Some(50.0));
        assert!(two_rows(50.0).to_scene(lib()).is_ok());
        // With titles, the height named is the one that renders, and a point less fails.
        let titled = |height: f64| {
            let mut f = two_rows(height);
            for row in 0..2 {
                f.axes_at_mut(GridIndex::At(row), GridIndex::At(0))
                    .unwrap()
                    .title("Load")
                    .subtitle("Daily mean");
            }
            f
        };
        let need = named_height(&render_error(&titled(60.0))).unwrap();
        assert!(titled(need).to_scene(lib()).is_ok());
        assert_eq!(named_height(&render_error(&titled(need - 1.0))), Some(need));

        // Texts wider than their panel: each plot of two columns without bands is
        // (width - 48) / 2 wide.
        let titled_columns = |width: f64, text: &str, which: usize| {
            let mut f = grid_figure(1, 2, (width / 72.0, 4.0), Theme::pigment());
            for col in 0..2 {
                let mut axes = add(&mut f, 0, col, 1, 1);
                axes.xticks(&[]).unwrap().yticks(&[]).unwrap();
                line(&mut axes);
            }
            let mut axes = f.axes_at_mut(GridIndex::At(0), GridIndex::At(1)).unwrap();
            match which {
                0 => axes.title(text),
                1 => axes.subtitle(text),
                2 => axes.xlabel(text),
                _ => axes.ylabel(text),
            };
            f
        };
        let text = "Station output";
        for (which, name, hint) in [
            (0, "the title", "shorten the title or widen the figure."),
            (
                1,
                "the subtitle",
                "shorten the subtitle or widen the figure.",
            ),
            (
                2,
                "the x axis label",
                "shorten the x axis label or widen the figure.",
            ),
            (
                3,
                "the y axis label",
                "shorten the y axis label or widen the figure.",
            ),
        ] {
            let probe = titled_columns(864.0, text, which);
            let width = {
                let (_, layout) = &layouts(&probe).unwrap()[1];
                [
                    &layout.title,
                    &layout.subtitle,
                    &layout.xlabel,
                    &layout.ylabel,
                ][which]
                    .as_ref()
                    .unwrap()
                    .width()
            };
            let boundary = 48.0 + 2.0 * width;
            let failing = boundary.ceil() - 1.0;
            let err = render_error(&titled_columns(failing, text, which));
            let room = ((failing - 48.0) / 2.0).floor();
            assert_eq!(
                err.to_string(),
                format!(
                    "Figure.to_scene(): row 0, col 1: {name} is {} pt wide; the panel has \
                     {room} pt.",
                    width.round().max(room + 1.0)
                )
            );
            assert_eq!(err.hint(), Some(hint));
            assert!(
                titled_columns(failing + 1.0, text, which)
                    .to_scene(lib())
                    .is_ok()
            );
        }
    }

    #[test]
    fn single_panel_room_errors_are_unchanged() {
        // A figure of one cell, made with or without a grid, fails as a single panel: no cell
        // in the message, the single panel's hints, and the same scene when it renders.
        let single = |grid: bool, width: f64, height: f64| {
            let mut f = if grid {
                grid_figure(1, 1, (width, height), Theme::pigment())
            } else {
                Figure::new(width, height, Theme::pigment()).unwrap()
            };
            let mut axes = f.add_axes(ScaleKind::Category, ScaleKind::Linear).unwrap();
            axes.title("Station output");
            f
        };
        /// A case: its name, a change to the figure, its size, and how the message ends and
        /// the hint.
        type Case = (
            &'static str,
            fn(&mut Figure),
            f64,
            f64,
            &'static str,
            &'static str,
        );
        let cases: [Case; 4] = [
            (
                "too short",
                |_| {},
                3.0,
                0.4,
                " pt is needed for the plot area.",
                "make the figure taller, or remove the title.",
            ),
            (
                "too narrow",
                |_| {},
                0.3,
                3.0,
                " pt is needed for the plot area.",
                "widen the figure.",
            ),
            (
                "y labels",
                |f| {
                    f.single_axes_mut()
                        .yticks_labeled(&[0.0, 1.0], vec!["0".into(), "W".repeat(40)])
                        .unwrap();
                },
                3.0,
                3.0,
                " pt leaves room for the plot area.",
                "shorten the y tick labels or widen the figure.",
            ),
            (
                "x labels",
                |f| {
                    f.single_axes_mut()
                        .bar(
                            vec!["W".repeat(30), "M".repeat(30)],
                            &[1.0, 2.0],
                            BarOptions::default(),
                        )
                        .unwrap();
                },
                3.0,
                3.0,
                " pt fits this figure.",
                "shorten the labels or widen the figure.",
            ),
        ];
        for (name, edit, width, height, ending, hint) in cases {
            let mut errors = Vec::new();
            for grid in [false, true] {
                let mut f = single(grid, width, height);
                edit(&mut f);
                let err = render_error(&f);
                let message = err.to_string();
                assert!(
                    message.starts_with("Figure.to_scene(): ") && !message.contains("row 0"),
                    "{name}: {message}"
                );
                assert!(message.ends_with(ending), "{name}: {message}");
                assert_eq!(err.hint(), Some(hint), "{name}");
                errors.push(message);
            }
            assert_eq!(errors[0], errors[1], "{name}");
        }
        let scene = |grid: bool| single(grid, 6.0, 4.0).to_scene(lib()).unwrap();
        assert_eq!(scene(false), scene(true));
    }

    #[test]
    fn x_labels_reach_into_the_gap_up_to_its_middle() {
        let lib = lib();
        for theme in themes() {
            let mut f = grid_figure(1, 3, (12.0, 4.0), theme);
            for col in 0..3 {
                let mut axes = add(&mut f, 0, col, 1, 1);
                line(&mut axes);
            }
            // The middle panel's last label reaches far right, its first far left; without y
            // tick labels, its column has no band the first label could reach under.
            f.axes_at_mut(GridIndex::At(0), GridIndex::At(1))
                .unwrap()
                .yticks(&[])
                .unwrap()
                .xticks_labeled(
                    &[0.0, 5.0, 10.0],
                    vec!["W".repeat(9), "5".into(), "W".repeat(9)],
                )
                .unwrap();
            let all = layouts(&f).unwrap();
            assert_aligned(&all).unwrap();
            let (left, middle, right) = (&all[0].1, &all[1].1, &all[2].1);
            let first = &middle.x_tick_labels[0];
            let last = &middle.x_tick_labels[2];
            // Each end label ends exactly in the middle of its gap, where the neighbour's frame
            // starts, and never reaches the neighbour's y tick labels.
            assert!((first.x - left.frame.right()).abs() < 1e-6, "{}", first.x);
            assert!((last.x + last.width() - right.frame.x).abs() < 1e-6);
            assert_eq!(middle.frame.x, left.frame.right());
            assert_eq!(middle.frame.right(), right.frame.x);
            let right_labels = right
                .y_tick_labels
                .iter()
                .map(|l| l.x)
                .fold(f64::INFINITY, f64::min);
            assert!(last.x + last.width() < right_labels);
            let _ = lib;
        }
    }

    #[test]
    fn edge_labels_inset_the_whole_grid() {
        // The right panel's last x tick label reaches past the canvas edge: every plot narrows
        // so that it ends X_LABEL_EDGE_GAP from the edge, and the plots stay equal.
        let mut f = grid_figure(1, 2, (12.0, 4.0), Theme::graphite());
        for col in 0..2 {
            line(&mut add(&mut f, 0, col, 1, 1));
        }
        f.axes_at_mut(GridIndex::At(0), GridIndex::At(1))
            .unwrap()
            .xticks_labeled(&[0.0, 10.0], vec!["0".into(), "W".repeat(6)])
            .unwrap();
        let all = layouts(&f).unwrap();
        assert_aligned(&all).unwrap();
        let last = &all[1].1.x_tick_labels[1];
        let end = last.x + last.width();
        assert!((end - (864.0 - X_LABEL_EDGE_GAP)).abs() < 1e-6, "{end}");
    }

    #[test]
    fn panel_rows_wrap_within_the_plot_and_direct_labels_follow_the_panel() {
        // A header legend row of a panel in one of three columns starts at its plot's left
        // edge and wraps within the plot, not the canvas.
        let grid = Grid::new(1, 3)
            .unwrap()
            .with_legend(crate::LegendMode::PerPanel);
        let mut f = Figure::with_grid(18.0, 6.0, Theme::pigment(), grid).unwrap();
        let mut axes = add(&mut f, 0, 1, 1, 1);
        axes.title("Stations");
        for i in 0..12 {
            axes.line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[0.0, f64::from(i)],
                LineOptions {
                    label: format!("Station {i}"),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        }
        let all = layouts(&f).unwrap();
        let layout = &all[0].1;
        let entries = layout.legend.entries();
        assert_eq!(entries.len(), 12);
        assert_eq!(entries[0].swatch.x, layout.plot.x);
        for entry in entries {
            assert!(entry.swatch.x >= layout.plot.x);
            assert!(entry.label.x + entry.label.width() <= layout.plot.right() + 1e-9);
        }
        assert!(entries.last().unwrap().label.y > entries[0].label.y);

        // Direct labels reserve at most a quarter of their cells, not of the canvas.
        let cell = (18.0 * 72.0 - 2.0 * PADDING - 2.0 * 24.0) / 3.0;
        crate::pipeline::with_layout_input(&f, (0, 1), lib(), |input, grid| {
            let span = grid.span(input.axes.cells());
            assert!((span.reserve_cap - END_LABEL_SHARE * cell).abs() < 1e-9);
            assert!((span.row_left - (PADDING + cell + 24.0)).abs() < 1e-9);
            assert!((span.row_right - span.row_left - cell).abs() < 1e-9);
        });
    }

    /// Columns with bands `left` and `right`, `gap` apart, on a `canvas` pt wide canvas.
    fn columns(canvas: f64, gap: f64, left: Vec<f64>, right: Vec<f64>) -> Columns {
        Columns {
            canvas,
            gap,
            left,
            right,
        }
    }

    /// The growth of entry `entry` that `placed` realizes over the bands of `cols`.
    fn realized(cols: &Columns, placed: &PlacedColumns, entry: usize) -> f64 {
        let last = cols.count() - 1;
        match entry {
            0 => placed.plot_x[0] - placed.left[0] - PADDING,
            1 => {
                cols.canvas
                    - PADDING
                    - (placed.plot_x[last] + placed.plot_width + placed.right[last])
            }
            g if g % 2 == 0 => placed.left[g / 2] - cols.left[g / 2],
            g => placed.right[(g - 3) / 2] - cols.right[(g - 3) / 2],
        }
    }

    /// Asserts that every label of `bounds` is within its limit on `placed`, that the plots are
    /// at least `MIN_PLOT` wide and that the canvas edges keep their caps.
    fn assert_fitted(
        cols: &Columns,
        placed: &PlacedColumns,
        bounds: &[LabelBound],
    ) -> std::result::Result<(), TestCaseError> {
        for b in bounds {
            let growth = realized(cols, placed, b.growth);
            prop_assert!(
                b.required(placed) <= growth + CONTAINMENT_TOLERANCE,
                "{b:?}: {} > {growth}",
                b.required(placed)
            );
        }
        prop_assert!(placed.plot_width >= MIN_PLOT - 1e-9);
        let (cap_left, cap_right) = cols.edge_caps();
        prop_assert!(realized(cols, placed, 0) <= cap_left + 1e-6);
        prop_assert!(realized(cols, placed, 1) <= cap_right + 1e-6);
        Ok(())
    }

    /// Builds labels around a known feasible growth and checks that the solver finds a fit.
    fn assert_known_columns_fit(
        count: usize,
        rows: &[Vec<usize>],
        bands: &[(f64, f64)],
        known: &[f64],
        labels: &[(f64, f64)],
        canvas: f64,
    ) -> std::result::Result<(), TestCaseError> {
        let cols = columns(
            canvas,
            24.0,
            bands.iter().take(count).map(|b| b.0).collect(),
            bands.iter().take(count).map(|b| b.1).collect(),
        );
        let (cap_left, cap_right) = cols.edge_caps();
        // A known growth that leaves the plots at least a tenth of their open width.
        let open = cols.place(&cols.no_growth()).plot_width;
        prop_assume!(open >= 10.0 * MIN_PLOT);
        let budget = 0.9 * open * count as f64 / (2 * count) as f64;
        let mut growth: Vec<f64> = known[..2 * count].iter().map(|k| k * budget).collect();
        growth[0] = growth[0].min(cap_left);
        growth[1] = growth[1].min(cap_right);
        let at = cols.place(&growth);
        prop_assume!(at.plot_width >= MIN_PLOT);
        // Panels: each row splits the columns into spans.
        let mut panels = Vec::new();
        for spans in rows {
            let mut first = 0;
            for span in spans {
                if first == count {
                    break;
                }
                let last = (first + span - 1).min(count - 1);
                panels.push((first, last));
                first = last + 1;
            }
        }
        // Labels of random panels, each as wide as the known growth allows, or less.
        let mut bounds = Vec::new();
        for (i, &(pos, room)) in labels.iter().enumerate() {
            let panel = i % panels.len();
            let (first, last) = panels[panel];
            let left_room = if first == 0 {
                PADDING - X_LABEL_EDGE_GAP
            } else {
                12.0
            };
            let right_room = if last + 1 == count {
                PADDING - X_LABEL_EDGE_GAP
            } else {
                12.0
            };
            let mut pair = [
                LabelBound {
                    panel,
                    first,
                    last,
                    growth: Columns::left_growth(first),
                    half: 0.0,
                    share: pos,
                    base: cols.left[first] + left_room,
                },
                LabelBound {
                    panel,
                    first,
                    last,
                    growth: Columns::right_growth(last, count),
                    half: 0.0,
                    share: 1.0 - pos,
                    base: cols.right[last] + right_room,
                },
            ];
            // The widest half each side allows at the known growth.
            let half = pair
                .iter()
                .map(|b| growth[b.growth] - b.required(&at))
                .fold(f64::INFINITY, f64::min);
            let half = (room * half).max(0.0);
            for b in &mut pair {
                b.half = half;
            }
            bounds.extend(pair);
        }
        assert_fitted(&cols, &at, &bounds)?;
        let fitted = fit_columns(&cols, &bounds);
        prop_assert!(fitted.is_ok(), "{bounds:?}");
        if let Ok(placed) = fitted {
            assert_fitted(&cols, &placed, &bounds)?;
            // The least total growth, up to the margin of the solve: no wider plots fit.
            prop_assert!(placed.plot_width >= at.plot_width - 1e-3);
        }
        Ok(())
    }

    #[test]
    fn twenty_two_columns_with_near_endpoint_labels_fit() {
        // Exact minimal input from CI run 38572056803.
        let rows = [
            vec![21, 1],
            vec![8, 14],
            vec![21, 1],
            vec![1],
            vec![22],
            vec![13, 1],
        ];
        let mut bands = [(0.0, 0.0); 32];
        bands[20].1 = 40.275609168231725;
        let mut known = [0.0; 64];
        for (entry, value) in [
            (1, 0.5081251091636685),
            (9, 0.6582789108880946),
            (11, 0.4101609823932938),
            (12, 0.7774890596712326),
            (13, 0.27058475710456104),
            (14, 0.31227338447421904),
            (15, 0.5928941082221074),
            (17, 0.5374834131344018),
            (20, 0.4991772170379112),
            (21, 0.5222050814267442),
            (23, 0.39686674796811133),
            (25, 0.9263579507935972),
            (28, 0.45789199295745026),
            (30, 0.5443759383258923),
            (31, 0.1277514848008628),
            (32, 0.7136848769034178),
            (33, 0.6995307228224282),
            (34, 0.8344355772876255),
            (35, 0.8235340934097671),
            (36, 0.7830738856340518),
            (37, 0.525008561176744),
            (38, 0.5823865439891766),
            (39, 0.02385390523752573),
            (40, 0.6164100151070087),
            (41, 0.755436155176224),
        ] {
            known[entry] = value;
        }
        let mut labels = [(0.0, 1.0); 34];
        for (label, pos) in [
            (0, 0.5),
            (2, 1e-10),
            (4, 0.5785756375468266),
            (8, 1e-10),
            (13, 0.9999999999),
            (17, 0.9999999999),
            (18, 0.001),
            (23, 1e-10),
            (32, 0.999),
        ] {
            labels[label].0 = pos;
        }
        assert_known_columns_fit(22, &rows, &bands, &known, &labels, 11218.060618880776).unwrap();
    }

    /// A tick position: often an end or within a hair of one.
    fn position() -> impl Strategy<Value = f64> {
        prop_oneof![
            2 => Just(0.0),
            2 => Just(1.0),
            1 => Just(0.5),
            1 => Just(1e-3),
            1 => Just(1.0 - 1e-3),
            1 => Just(1e-10),
            1 => Just(1.0 - 1e-10),
            4 => 0.0..=1.0_f64,
        ]
    }

    /// The share of the widest label the known growth fits that a label takes: often all of
    /// it, so that the label's bound is tight at the known growth.
    fn tightness() -> impl Strategy<Value = f64> {
        prop_oneof![2 => Just(1.0), 1 => 0.0..=1.0_f64]
    }

    proptest! {
        // Apply the environment after these defaults so stress runs can override them.
        #![proptest_config(proptest::test_runner::contextualize_config(ProptestConfig {
            cases: 2_000,
            rng_seed: proptest::test_runner::RngSeed::Fixed(38572056803),
            max_global_rejects: 1_024,
            ..ProptestConfig::default()
        }))]

        #[test]
        fn fitted_columns_keep_every_label_in_bounds(
            spans in prop::collection::vec(1_usize..=3, 1..=4),
            bands in prop::collection::vec((0.0..60.0_f64, 0.0..60.0_f64), 12),
            labels in prop::collection::vec(
                prop::collection::vec((0.0..=1.0_f64, 0.0..120.0_f64), 0..4),
                4,
            ),
            canvas in 100.0..1500.0_f64,
        ) {
            let count: usize = spans.iter().sum();
            let cols = columns(
                canvas,
                24.0,
                bands.iter().take(count).map(|b| b.0).collect(),
                bands.iter().take(count).map(|b| b.1).collect(),
            );
            prop_assume!(cols.place(&cols.no_growth()).plot_width >= MIN_PLOT);
            let mut bounds = Vec::new();
            let mut first = 0;
            for (panel, (span, labels)) in spans.iter().zip(&labels).enumerate() {
                let last = first + span - 1;
                let left_room = if first == 0 { PADDING - X_LABEL_EDGE_GAP } else { 12.0 };
                let right_room = if last + 1 == count { PADDING - X_LABEL_EDGE_GAP } else { 12.0 };
                for &(pos, half) in labels {
                    bounds.push(LabelBound {
                        panel, first, last,
                        growth: Columns::left_growth(first),
                        half, share: pos,
                        base: cols.left[first] + left_room,
                    });
                    bounds.push(LabelBound {
                        panel, first, last,
                        growth: Columns::right_growth(last, count),
                        half, share: 1.0 - pos,
                        base: cols.right[last] + right_room,
                    });
                }
                first = last + 1;
            }
            if let Ok(placed) = fit_columns(&cols, &bounds) {
                assert_fitted(&cols, &placed, &bounds)?;
            }
        }

        /// Problems built around a known growth that fits every label are never refused:
        /// panels in several rows whose column spans overlap, labels anywhere along them, each
        /// within its bound at the known growth.
        #[test]
        fn columns_that_can_fit_their_labels_are_fitted(
            count in 1_usize..=32,
            rows in prop::collection::vec(prop::collection::vec(1_usize..=32, 1..=32), 1..=6),
            bands in prop::collection::vec((0.0..60.0_f64, 0.0..60.0_f64), 32),
            known in prop::collection::vec(0.0..1.0_f64, 64),
            labels in prop::collection::vec((position(), tightness()), 1..=60),
            canvas in 300.0..14_400.0_f64,
        ) {
            assert_known_columns_fit(count, &rows, &bands, &known, &labels, canvas)?;
        }
    }

    #[test]
    fn spanning_panels_use_the_bands_between_their_columns() {
        // Two columns of a 1000 pt canvas without bands. The top panel spans both, with a
        // label at 0.1 reaching far left; the bottom-left panel's last label reaches far into
        // the gap. Growing the band right of column 0 for the bottom label keeps the top
        // panel's plot as wide (it covers that band), so the left edge needs only a little
        // growth: about 219 pt, under the 244 pt cap.
        let cols = columns(1000.0, 24.0, vec![0.0; 2], vec![0.0; 2]);
        let bound = |panel, first, last, growth, half, share, base| LabelBound {
            panel,
            first,
            last,
            growth,
            half,
            share,
            base,
        };
        let bounds = vec![
            bound(0, 0, 1, 0, 300.0, 0.1, 6.0),
            bound(0, 0, 1, 1, 300.0, 0.9, 6.0),
            bound(1, 0, 0, 0, 312.0, 1.0, 6.0),
            bound(1, 0, 0, 3, 312.0, 0.0, 12.0),
        ];
        let placed = fit_columns(&cols, &bounds).unwrap();
        assert_fitted(&cols, &placed, &bounds).unwrap();
        assert!((realized(&cols, &placed, 3) - 300.0).abs() < 1e-6);
        assert!((realized(&cols, &placed, 0) - 218.0 - 2.0 / 9.0).abs() < 1e-6);

        // The same through the figure, with real labels.
        let mut f = grid_figure(2, 2, (1000.0 / 72.0, 600.0 / 72.0), Theme::pigment());
        for (row, span, tick, count) in [(0, 2, 0.1, 79), (1, 1, 1.0, 82)] {
            let mut axes = add(&mut f, row, 0, 1, span);
            axes.xlim(0.0, 1.0)
                .unwrap()
                .yticks(&[])
                .unwrap()
                .xticks_labeled(&[tick], vec!["W".repeat(count)])
                .unwrap();
            line(&mut axes);
        }
        let all = layouts(&f).unwrap();
        assert_aligned(&all).unwrap();
        let top = &all[0].1.x_tick_labels[0];
        let bottom = &all[1].1.x_tick_labels[0];
        assert!((top.x - X_LABEL_EDGE_GAP).abs() < 1e-6, "{}", top.x);
        assert!((bottom.x + bottom.width() - all[1].1.frame.right()).abs() < 1e-6);
    }

    #[test]
    fn texts_are_checked_on_the_final_layout() {
        // 410 × 200 pt, font scale 2: the first pass's y tick labels (0.00, 0.25, …) leave the
        // plots about 136 pt wide, the final ones (0.0, 0.5, 1.0) about 146 pt, and a title
        // of 146 pt fits only the final plots.
        let theme = Theme::pigment().with_font_scale(2.0).unwrap();
        let mut f = grid_figure(1, 2, (410.0 / 72.0, 200.0 / 72.0), theme);
        for col in 0..2 {
            let mut axes = add(&mut f, 0, col, 1, 1);
            axes.title("WWWWW").ylim(0.0, 1.0).unwrap();
            line(&mut axes);
        }
        let all = layouts(&f).unwrap();
        let (_, layout) = &all[0];
        let title = layout.title.as_ref().unwrap().width();
        assert!(title <= layout.plot.width && title > 140.0, "{title}");
    }

    #[test]
    fn x_axis_labels_of_a_row_line_up() {
        // One panel without x tick labels, one with: their x axis labels share a baseline at
        // the bottom of the row's band.
        for theme in themes() {
            let mut f = grid_figure(1, 2, (12.0, 4.0), theme);
            for col in 0..2 {
                let mut axes = add(&mut f, 0, col, 1, 1);
                axes.title("Title").xlabel("Time");
                if col == 0 {
                    axes.xticks(&[]).unwrap();
                }
                line(&mut axes);
            }
            let all = layouts(&f).unwrap();
            let baseline = |i: usize| all[i].1.xlabel.as_ref().unwrap().y;
            assert!((baseline(0) - baseline(1)).abs() < 1e-9);
            assert_eq!(all[0].1.plot.bottom(), all[1].1.plot.bottom());
            assert!(baseline(1) > all[1].1.x_tick_labels[0].y);
        }
    }

    #[test]
    fn bottom_y_tick_labels_stay_in_their_row() {
        // At font scale 10 the bottom y tick label of the upper panel reaches far below its
        // plot; without x tick labels or an x axis label, the band below the row makes room.
        let theme = Theme::pigment().with_font_scale(10.0).unwrap();
        let mut f = grid_figure(2, 1, (12.0, 8.0), theme);
        for row in 0..2 {
            let mut axes = add(&mut f, row, 0, 1, 1);
            axes.xticks(&[])
                .unwrap()
                .yticks_labeled(&[0.0, 1.0], vec!["g".into(), "1".into()])
                .unwrap();
            line(&mut axes);
        }
        let all = layouts(&f).unwrap();
        let upper = &all[0].1;
        let label = &upper.y_tick_labels[0];
        let descent = f64::from(label.layout.descent());
        assert!(label.y + descent <= upper.frame.bottom() + 1e-9);
        assert!(upper.frame.bottom() <= all[1].1.frame.y + 1e-9);
    }

    #[test]
    fn reported_sizes_pass_the_room_checks() {
        // Seven rows, row r holding one panel from column 0 spanning r + 1 columns, each with
        // a long direct label: the room the labels reserve grows with the canvas, so the
        // width needed is found by its segments, and a figure that wide renders.
        let build = |width: f64| {
            let mut f = grid_figure(7, 7, (width / 72.0, 20.0), Theme::graphite());
            for row in 0..7 {
                add(&mut f, row, 0, 1, row + 1)
                    .xticks(&[])
                    .unwrap()
                    .yticks(&[])
                    .unwrap()
                    .line(
                        AxisData::Numbers(vec![0.0, 1.0]),
                        &[0.0, 1.0],
                        LineOptions {
                            label: "W".repeat(300),
                            ..LineOptions::default()
                        },
                    )
                    .unwrap();
            }
            f
        };
        let need = named_width(&render_error(&build(100.0))).unwrap();
        let wide = build(need);
        match wide.to_scene(lib()) {
            Ok(_) => {}
            Err(err) => assert!(named_width(&err).is_none(), "{err}"),
        }
        let narrower = render_error(&build(need - 1.0));
        assert_eq!(named_width(&narrower), Some(need), "{narrower}");

        // Heights: rows with titles, subtitles and wrapped legend rows.
        let tall = |height: f64| {
            let mut f = grid_figure(3, 2, (8.0, height / 72.0), Theme::midnight());
            for row in 0..3 {
                for col in 0..2 {
                    let mut axes = add(&mut f, row, col, 1, 1);
                    axes.title("Load").subtitle("Daily mean");
                    for i in 0..4 {
                        axes.line(
                            AxisData::Numbers(vec![0.0, 1.0]),
                            &[0.0, f64::from(i)],
                            LineOptions {
                                label: format!("Station {i}"),
                                ..LineOptions::default()
                            },
                        )
                        .unwrap();
                    }
                }
            }
            f
        };
        let need = named_height(&render_error(&tall(100.0))).unwrap();
        if let Err(err) = tall(need).to_scene(lib()) {
            assert!(named_height(&err).is_none(), "{err}");
        }
        assert_eq!(named_height(&render_error(&tall(need - 1.0))), Some(need));
    }

    #[test]
    fn legend_labels_stay_in_their_panel() {
        let build = |theme: Theme, label: String| {
            let grid = Grid::new(1, 2)
                .unwrap()
                .with_legend(crate::LegendMode::PerPanel);
            let mut f = Figure::with_grid(12.0, 4.0, theme, grid).unwrap();
            for col in 0..2 {
                let mut axes = add(&mut f, 0, col, 1, 1);
                axes.xlim(0.0, 1.0).unwrap();
                axes.line(
                    AxisData::Numbers(vec![0.0, 1.0]),
                    &[0.0, 1.0],
                    LineOptions {
                        label: if col == 0 {
                            label.clone()
                        } else {
                            String::new()
                        },
                        ..LineOptions::default()
                    },
                )
                .unwrap();
            }
            f
        };
        let long = "W".repeat(90);
        // A direct label that would end past the panel's frame.
        let err = render_error(&build(Theme::graphite(), long.clone()));
        let message = err.to_string();
        assert!(
            message.starts_with(&format!(
                "Figure.to_scene(): row 0, col 0: the direct label \"{}…",
                "W".repeat(38)
            )),
            "{message}"
        );
        assert!(message.contains(" pt fits beside the plot."), "{message}");
        assert_eq!(err.hint(), Some("shorten the label or widen the figure."));
        // A header row entry wider than the plot, and a chip.
        for theme in [Theme::pigment(), Theme::midnight()] {
            let err = render_error(&build(theme, long.clone()));
            let message = err.to_string();
            assert!(
                message.starts_with("Figure.to_scene(): row 0, col 0: the legend label \""),
                "{message}"
            );
            assert!(message.contains(" pt wide; the panel has "), "{message}");
            assert_eq!(err.hint(), Some("shorten the label or widen the figure."));
        }
        // Short labels render, inside their panel.
        for theme in themes() {
            let all = layouts(&build(theme, "North".into())).unwrap();
            let (left, right) = (&all[0].1, &all[1].1);
            for entry in left.legend.entries() {
                assert!(entry.label.x + entry.label.width() <= left.plot.right());
            }
            for label in left.direct.iter().filter_map(|d| d.label.as_ref()) {
                assert!(label.x + label.width() <= left.frame.right());
                assert!(label.x + label.width() < right.plot.x);
            }
        }
    }

    /// A 14 400 pt grid of 21 columns with 28 label bounds whose widths are exact `f32` values
    /// and whose ticks lie at the ends and within 0.001 of them: line 1 the canvas and gap,
    /// lines 2 and 3 the left and right bands, line 4 a growth that fits every bound, then one
    /// bound per line (panel, first and last column, growth entry, half width, share, base).
    const TWENTY_ONE_COLUMNS: &str = "\
14400,24\n\
2.02018184661865234e1,2.46706352233886719e1,1.65958366394042969e1,6.18513131141662598e0,1.66243724822998047e1,2.25761775970458984e1,1.48729505538940430e1,1.71191406250000000e1,2.12288761138916016e1,1.93691272735595703e1,1.72622661590576172e1,1.48594293594360352e1,1.07971272468566895e1,1.08702840805053711e1,1.98756189346313477e1,9.39824867248535156e0,2.27736587524414062e1,9.49333143234252930e0,6.96388614177703857e0,1.87588129043579102e1,6.26450407505035400e0\n\
2.63816623687744141e1,1.09761743545532227e1,1.85272541046142578e1,1.99630889892578125e1,2.09487733840942383e1,2.65384197235107422e1,2.34401025772094727e1,2.40130672454833984e1,8.03261146321892738e0,2.42730255126953125e1,2.37710456848144531e1,2.54840793609619141e1,2.39509229660034180e1,1.45349411964416504e1,9.72839486598968506e0,2.58556365966796875e1,1.40061492919921875e1,2.03323516845703125e1,1.90521755218505859e1,1.32186889648437500e1,8.76106053590774536e0\n\
0.00000000000000000e0,0.00000000000000000e0,7.64948085458930649e0,9.66551922206344472e0,9.96614885680216034e0,1.06245092432491379e1,4.74997017550360745e0,1.98370945485611081e1,4.35813898827932178e0,2.61766192688434494e1,4.26609117057946907e0,1.32715953911510720e1,1.26970225347257433e1,7.56322082725313649e-1,4.20899973252741866e-2,1.26911654483521392e1,2.57794843927259230e1,2.61167419112370318e1,1.77297054469745134e1,2.80688000327603753e1,1.89939539580475625e1,1.66890622914523625e1,1.66962180349646871e1,1.29828375417464024e1,1.76145028752550274e1,1.60755120633935107e1,2.14979803730139132e1,1.87471014381564665e1,2.87704096798247875e1,1.52652646950178177e1,8.56823328620942526e0,1.84539040749560783e1,1.73598404200942262e1,1.75041177849328662e1,4.21144539897317927e-1,2.56685015087456385e1,2.60135626315160309e1,8.14198780874221262e-1,2.47681049016567307e1,1.57365273060361410e-1,5.01797070913894050e0,1.89054397895198534e1\n\
0,0,7,0,6.74946517944335938e1,9.98999999999999999e-1,2.62018184661865234e1\n\
0,0,7,17,6.74946517944335938e1,1.00000000000000089e-3,3.60130672454833984e1\n\
1,8,12,16,3.99343109130859375e1,1.00000000000000002e-3,3.32288761138916016e1\n\
1,8,12,27,3.99343109130859375e1,9.98999999999999999e-1,3.59509229660034180e1\n\
2,13,14,26,1.59284105300903320e1,1.00000000000000002e-3,2.28702840805053711e1\n\
2,13,14,31,1.59284105300903320e1,9.98999999999999999e-1,2.17283948659896851e1\n\
2,13,14,26,3.26839111328125000e2,6.43493104085467338e-1,2.28702840805053711e1\n\
2,13,14,31,3.26839111328125000e2,3.56506895914532662e-1,2.17283948659896851e1\n\
3,15,18,30,3.26230239868164062e1,1.00000000000000002e-3,2.13982486724853516e1\n\
3,15,18,39,3.26230239868164062e1,9.98999999999999999e-1,3.10521755218505859e1\n\
4,19,19,38,4.65982398986816406e1,1.00000000000000002e-3,3.07588129043579102e1\n\
4,19,19,41,4.65982398986816406e1,9.98999999999999999e-1,2.52186889648437500e1\n\
4,19,19,38,4.47234611511230469e1,9.98999999999999999e-1,3.07588129043579102e1\n\
4,19,19,41,4.47234611511230469e1,1.00000000000000089e-3,2.52186889648437500e1\n\
11,13,16,26,5.80263244628906250e2,8.02575732939278752e-1,2.28702840805053711e1\n\
11,13,16,35,5.80263244628906250e2,1.97424267060721248e-1,2.60061492919921875e1\n\
11,13,16,26,9.25740905761718750e2,6.73542175847730906e-1,2.28702840805053711e1\n\
11,13,16,35,9.25740905761718750e2,3.26457824152269094e-1,2.60061492919921875e1\n\
13,0,15,0,5.49470996093750000e3,5.00000000000000000e-1,2.62018184661865234e1\n\
13,0,15,33,5.49470996093750000e3,5.00000000000000000e-1,3.78556365966796875e1\n\
18,7,14,14,1.40952835083007812e2,9.81684820633810018e-1,2.91191406250000000e1\n\
18,7,14,31,1.40952835083007812e2,1.83151793661899820e-2,2.17283948659896851e1\n\
23,15,16,30,4.94252967834472656e1,1.00000000000000000e0,2.13982486724853516e1\n\
23,15,16,35,4.94252967834472656e1,0.00000000000000000e0,2.60061492919921875e1\n\
23,15,16,30,5.29808120727539062e1,9.98999999999999999e-1,2.13982486724853516e1\n\
23,15,16,35,5.29808120727539062e1,1.00000000000000089e-3,2.60061492919921875e1\n\
24,17,18,34,6.48849853515625000e2,5.16256245220348520e-1,2.14933314323425293e1\n\
24,17,18,39,6.48849853515625000e2,4.83743754779651480e-1,3.10521755218505859e1";

    #[test]
    fn a_known_fitting_growth_of_21_columns_is_found() {
        let mut lines = TWENTY_ONE_COLUMNS.lines();
        let mut numbers = || -> Vec<f64> {
            lines
                .next()
                .unwrap()
                .split(',')
                .map(|x| x.parse().unwrap())
                .collect()
        };
        let head = numbers();
        let (left, right, known) = (numbers(), numbers(), numbers());
        let cols = columns(head[0], head[1], left, right);
        let bounds: Vec<LabelBound> = TWENTY_ONE_COLUMNS
            .lines()
            .skip(4)
            .map(|line| {
                let n: Vec<f64> = line.split(',').map(|x| x.parse().unwrap()).collect();
                LabelBound {
                    panel: n[0] as usize,
                    first: n[1] as usize,
                    last: n[2] as usize,
                    growth: n[3] as usize,
                    half: n[4],
                    share: n[5],
                    base: n[6],
                }
            })
            .collect();
        assert_eq!((cols.count(), bounds.len()), (21, 28));
        // The known growth fits: every bound, both edge caps and the smallest plot width.
        let at = cols.place(&known);
        let (cap_left, cap_right) = cols.edge_caps();
        assert!(known[0] <= cap_left && known[1] <= cap_right && at.plot_width >= MIN_PLOT);
        for b in &bounds {
            assert!(b.required(&at) <= known[b.growth] + 1e-9, "{b:?}");
        }
        let placed = fit_columns(&cols, &bounds).unwrap();
        assert_fitted(&cols, &placed, &bounds).unwrap();
        assert!(placed.plot_width >= at.plot_width - 1e-3);
    }

    #[test]
    fn widths_reported_below_the_zero_cell_width_are_the_least() {
        // 1 × 3 with one axes over two columns and a short direct label: below 72 pt the cells
        // are 0 wide, and the label's cap stays 6 pt until then. The least width is 83 pt.
        let build = |width: f64| {
            let mut f = grid_figure(1, 3, (width / 72.0, 4.0), Theme::graphite());
            add(&mut f, 0, 0, 1, 2)
                .xticks(&[])
                .unwrap()
                .yticks(&[])
                .unwrap()
                .xlim(0.0, 1.0)
                .unwrap()
                .line(
                    AxisData::Numbers(vec![0.0, 1.0]),
                    &[0.0, 1.0],
                    LineOptions {
                        label: "W".into(),
                        ..LineOptions::default()
                    },
                )
                .unwrap();
            f
        };
        let err = render_error(&build(48.0));
        assert_eq!(
            err.to_string(),
            "Figure.to_scene(): the figure is 48 pt wide; at least 83 pt is needed for 3 \
             columns of plots."
        );
        assert!(build(83.0).to_scene(lib()).is_ok());
        assert_eq!(named_width(&render_error(&build(82.0))), Some(83.0));
    }

    #[test]
    fn reported_widths_and_heights_are_the_least_that_fit() {
        // Random grids of spanning panels with direct labels of random widths: the width named
        // passes the width check, one point less does not, and no whole width between the
        // figure's and it passes. Then random row bands: the height named passes and one point
        // less does not.
        let mut rng = Rng(0x785a_b123_789d);
        for case in 0..120 {
            let count = 2 + rng.below(7);
            let initial = 25.0 + rng.below(80) as f64;
            let mut f = grid_figure(
                count as i64,
                count as i64,
                (initial / 72.0, 20.0),
                Theme::graphite(),
            );
            for row in 0..count {
                let span = 1 + rng.below(count);
                let col = rng.below(count - span + 1);
                add(&mut f, row as i64, col as i64, 1, span as i64)
                    .xticks(&[])
                    .unwrap()
                    .yticks(&[])
                    .unwrap()
                    .line(
                        AxisData::Numbers(vec![0.0, 1.0]),
                        &[0.0, 1.0],
                        LineOptions {
                            label: "W".repeat(1 + rng.below(140)),
                            ..LineOptions::default()
                        },
                    )
                    .unwrap();
            }
            let first = f.panels().next().unwrap().cells();
            crate::pipeline::with_layout_input(&f, (first.row(), first.col()), lib(), |_, grid| {
                let bands: Vec<PanelBands> = grid
                    .panels
                    .iter()
                    .map(|input| {
                        let marks = crate::resolve::resolve_marks(input.axes.marks(), input.theme);
                        let limits = crate::resolve::Limits::of(input.axes);
                        let (x, y) = super::super::TickRoom::unlimited(3, 3, lib());
                        let resolved = crate::resolve::resolve(&marks, &limits, &x, &y);
                        super::super::measure_panel(
                            input,
                            &resolved,
                            grid.span(input.axes.cells()),
                            [true; 2],
                        )
                    })
                    .collect();
                let cells: Vec<CellRange> = grid.panels.iter().map(|p| p.axes.cells()).collect();
                let columns = Columns::measure(grid, &cells, &bands);
                if columns.place(&columns.no_growth()).plot_width >= MIN_PLOT {
                    return;
                }
                let need = needed_width(grid, &cells, &columns);
                let width_at = |width: f64| {
                    let wider = GridInput { width, ..*grid };
                    let fresh: Vec<PanelBands> = grid
                        .panels
                        .iter()
                        .zip(&bands)
                        .map(|(p, b)| {
                            let mut b = b.clone();
                            b.direct_reserve = p
                                .legend
                                .direct
                                .reserve(wider.span(p.axes.cells()).reserve_cap);
                            b
                        })
                        .collect();
                    let c = Columns::measure(&wider, &cells, &fresh);
                    c.place(&c.no_growth()).plot_width
                };
                assert!(width_at(need) >= MIN_PLOT, "case {case}: {need}");
                assert!(width_at(need - 1.0) < MIN_PLOT, "case {case}: {need}");
                let least = (initial.floor() + 1.0) as usize;
                let brute = (least..=need as usize).find(|&w| width_at(w as f64) >= MIN_PLOT);
                assert_eq!(brute, Some(need as usize), "case {case}");
            });
        }
        let theme = Theme::pigment();
        for case in 0..300 {
            let rows = 2 + rng.below(31);
            let bands = Rows {
                gap: 24.0,
                above: (0..rows).map(|_| rng.unit() * 300.0).collect(),
                below: (0..rows).map(|_| rng.unit() * 100.0).collect(),
                legends: vec![0.0; rows],
            };
            let grid = Grid::new(rows as i64, 2).unwrap();
            let input = GridInput {
                width: 1000.0,
                height: 1.0,
                top: PADDING,
                grid: &grid,
                theme: &theme,
                context: "Figure.to_scene()",
                panels: &[],
            };
            let need = named_height(&bands.clone().place(&input).unwrap_err()).unwrap();
            let at = |height: f64| bands.clone().place(&GridInput { height, ..input });
            assert!(at(need).is_ok(), "case {case}");
            assert!(at(need - 1.0).is_err(), "case {case}");
        }
    }
}
