//! The figure model and the calls that render it.
//!
//! A [`Figure`] holds a [`Grid`] and the [`Axes`] placed on it, which hold [`Mark`]s; an
//! [`AxesMut`] changes one of them. Every input is validated at the call
//! that receives it, so a model that exists is always drawable. Marks hold copies of their data
//! that never change, so clones of a figure share them.
//! Colors stay as [`ColorSpec`](crate::ColorSpec)s until rendering resolves them against the
//! theme's palette. [`Figure::to_scene`], [`Figure::to_png`] and [`Figure::to_svg`] run the
//! render pipeline: resolve scales, ticks and mark colors, lay out the axes, then draw.
//!
//! # Examples
//!
//! ```
//! use umber_core::figure::{AxisData, Figure, LineOptions};
//! use umber_core::{ScaleKind, Theme};
//!
//! let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
//! figure
//!     .add_axes(ScaleKind::Linear, ScaleKind::Linear)?
//!     .title("Growth")
//!     .line(AxisData::Numbers(vec![0.0, 1.0, 2.0]), &[1.0, 3.0, 2.0], LineOptions::default())?;
//! assert_eq!(figure.panels().map(|a| a.marks().len()).sum::<usize>(), 1);
//! # Ok::<(), umber_core::Error>(())
//! ```

pub use crate::model::StackedAreaMark;
pub use crate::model::{
    AreaMark, AreaOptions, Axes, AxesMut, AxisData, BarGroupMark, BarMark, BarOptions, Cell,
    CellRange, ExplicitTicks, Figure, Grid, GridIndex, HistMark, HistOptions, LegendMode, LineMark,
    LineOptions, MAX_GRID, Mark, Placement, ScatterMark, ScatterOptions, Series, SeriesView, Share,
    Step, StepMark, StepOptions,
};
pub use crate::model::{AxisValue, HLineMark, RuleOptions, SpanMark, SpanOptions, VLineMark};
pub use crate::model::{BandMark, BandOptions};
pub use crate::model::{BoxGroupMark, BoxMark, BoxOptions, Distribution, DistributionView};
pub use crate::model::{BubbleMark, BubbleOptions};
pub use crate::model::{CmapChoice, ColorBy, PointColor};
pub use crate::model::{ErrorBarMark, ErrorBarOptions, ErrorBarOrientation};
pub use crate::model::{HeatmapMark, HeatmapOptions};
pub use crate::model::{StemMark, StemOptions};
pub use crate::model::{ViolinGroupMark, ViolinMark, ViolinOptions};

use crate::render::{SvgText, check_dpi, render_png, render_svg};
use crate::scene::Scene;
use crate::{FontLibrary, Result, pipeline, resolve};

impl Figure {
    /// Builds the figure's [`Scene`]: resolve scales and ticks, lay out the axes, then draw.
    /// The scene is `width × 72` by `height × 72` pt on the theme background. For each placed
    /// axes, in row-major order of their top-left cells, it holds the axes chrome (gridlines,
    /// tick labels, header and axis labels) as one unclipped group, followed by the visible
    /// marks, in insertion order, as one group clipped to the plot area (widened horizontally
    /// by half of the widest line stroke when the axes hold lines or areas, as far as the room
    /// beside the plot allows, so line ends stay whole), the values written on annotated
    /// heatmap cells as one group clipped to the plot area when there are any, the legend as
    /// one unclipped group when it shows anything, and last the baseline as an unclipped group
    /// of its own, so marks never cover it; two category axes have no value axis and draw no
    /// baseline. A non-empty figure header follows as one unclipped group: the figure
    /// title, subtitle and merged legend. Without axes it holds the background and any figure
    /// title or subtitle. The same figure always gives the same scene.
    ///
    /// A 1 × 1 grid is laid out on the whole canvas. On a larger grid every axes' plot area
    /// has the same size and the axes line up: each grid column has room left of its plots
    /// for the widest y tick labels among its axes and right of them for direct labels, each
    /// grid row has room above its plots for the tallest header, y axis label and rising top
    /// y tick label, and below them for the x tick labels and x axis labels, and the theme's
    /// [`panel_gap`](crate::Theme::panel_gap) separates neighbouring cells. An axes spanning
    /// several cells covers their plots and everything between them. An empty cell keeps its
    /// share of the canvas and draws nothing. Panel titles and subtitles align over their
    /// span using [`Theme::title_align`](crate::Theme::title_align); panel legend rows stay
    /// left-aligned and wrap within it. The figure title, subtitle and merged legend align
    /// within the canvas padding above the grid. Their height joins the grid's height need.
    /// The x axis labels of a grid row share a baseline. An x tick label may reach halfway
    /// into the gap next to its axes; beyond that, the plots of the grid narrow so that it fits.
    ///
    /// Automatic colors share one palette slot per label, in first appearance order across
    /// row-major panels. Unlabelled automatic series take slots after all label slots, per
    /// panel. Explicit colors do not consume slots. On a grid with a merged legend, entries
    /// with the same label and swatch kind merge, keeping the first swatch. Direct line labels
    /// remain on their panels. A 1 × 1 grid keeps its panel legend in either legend mode.
    ///
    /// # Errors
    ///
    /// `Error::Style` with context `"Figure.to_scene()"` when the plot area does not fit or its
    /// x tick labels cannot fit. The plot area is at least 1 pt wide, with or without x tick
    /// labels: when the y tick labels leave it less, the message names the widest y tick label
    /// and the hint suggests shortening the y tick labels or widening the figure; when even
    /// y tick labels of no width would, the message names the width the figure needs and the
    /// hint suggests widening it. Every x tick label keeps half the outer padding from the
    /// canvas edges, and the plot area moves in for them by at most a quarter of the width
    /// inside the padding on each side; labels that need more name the widest x tick label,
    /// and the hint suggests shortening the labels or widening the figure. The plot area is at
    /// least 1 pt tall too: when the header (title, subtitle and legend row), the axis labels,
    /// the x tick labels and the padding leave it less, the message names the height the
    /// figure needs, and the hint suggests making the figure taller or removing whichever of
    /// the title, subtitle and legend it has.
    ///
    /// Bubbles never make rendering fail: automatic limits fit whole bubble markers and their
    /// outlines to the final plot size, across every panel sharing the scale, and when they
    /// cannot (see [`AxesMut::bubbles`](crate::AxesMut::bubbles)) the axis keeps the limits of
    /// the data positions and the plot clips the bubbles.
    ///
    /// On a grid of more than one cell the same checks run for the grid's columns and rows, in
    /// the same order. A figure too narrow for its columns of plots names the width it needs
    /// (`the figure is … pt wide; at least … pt is needed for 3 columns of plots.`) unless
    /// the y tick labels of one column are the cause; one too short names the height
    /// (`… pt is needed for 2 rows of plots.`). Errors about one axes' texts start with
    /// `row {r}, col {c}: `, the axes' top-left cell: its y tick labels, its x tick labels,
    /// and texts that would run into the next cell: a title, subtitle, legend row entry or axis
    /// label wider than its plot area (`row 0, col 1: the title is … pt wide; the panel has …
    /// pt.`), or a direct label that would end past the axes' share of the canvas. A merged
    /// legend entry wider than the inner canvas also fails, naming its label and suggesting
    /// shortening the label or widening the figure.
    ///
    /// `Error::Data` in the same context, without a parameter, when a vertical reference line
    /// ([`AxesMut::vline`]) on a category x axis names a category that no other mark of its axes
    /// has. This is checked before any axes is laid out, and so is found even when another axes
    /// does not fit; on a grid the message starts with the axes' cell as above.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, FontLibrary, GridIndex, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
    /// assert!(figure.to_scene(FontLibrary::global())?.items.is_empty());
    /// figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?.title("Growth");
    /// let scene = figure.to_scene(FontLibrary::global())?;
    /// assert_eq!((scene.width, scene.height), (432.0, 288.0));
    /// assert!(scene.leaf_count() > 0);
    ///
    /// // A tick label far wider than the figure cannot fit.
    /// figure
    ///     .axes_at_mut(GridIndex::Auto, GridIndex::Auto)?
    ///     .xticks_labeled(&[0.0, 1.0], vec!["0".into(), "W".repeat(60)])?;
    /// let err = figure.to_scene(FontLibrary::global()).unwrap_err();
    /// assert!(err.to_string().starts_with("Figure.to_scene(): x tick label \"WWW"));
    /// assert_eq!(err.hint(), Some("shorten the labels or widen the figure."));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn to_scene(&self, library: &FontLibrary) -> Result<Scene> {
        self.scene(library, "Figure.to_scene()")
    }

    /// Renders the figure as a PNG at `dpi`, with the global [`FontLibrary`].
    ///
    /// Rendering may use up to 8 threads (see [`render_png`]).
    ///
    /// # Errors
    ///
    /// `Error::Style` with context `"Figure.to_png()"` when `dpi` is outside `10..=2400` (the
    /// message names the value however large or negative it is), when the plot area or its x
    /// tick labels cannot fit (see [`Figure::to_scene`]) or when the image would be too large
    /// (see [`render_png`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, Theme};
    ///
    /// let mut figure = Figure::new(2.0, 1.0, Theme::graphite())?;
    /// figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?;
    /// let png = figure.to_png(72)?;
    /// assert_eq!(&png[1..4], b"PNG");
    /// assert!(figure.to_png(1).is_err());
    /// assert!(figure.to_png(-72).is_err());
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn to_png(&self, dpi: i64) -> Result<Vec<u8>> {
        let dpi = check_dpi("Figure.to_png()", dpi)?;
        let scene = self.scene(FontLibrary::global(), "Figure.to_png()")?;
        render_png(&scene, dpi)
    }

    /// Renders the figure as an SVG document, with the global [`FontLibrary`].
    ///
    /// # Errors
    ///
    /// `Error::Style` with context `"Figure.to_svg()"` when the plot area or its x tick labels
    /// cannot fit (see [`Figure::to_scene`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Figure, ScaleKind, SvgText, Theme};
    ///
    /// let mut figure = Figure::new(6.0, 4.0, Theme::midnight())?;
    /// figure.add_axes(ScaleKind::Linear, ScaleKind::Linear)?.title("Signal");
    /// let svg = figure.to_svg(SvgText::Editable)?;
    /// assert!(svg.contains(r#"width="432pt" height="288pt""#));
    /// assert!(svg.contains(">Signal</text>"));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn to_svg(&self, text: SvgText) -> Result<String> {
        let scene = self.scene(FontLibrary::global(), "Figure.to_svg()")?;
        Ok(render_svg(&scene, text))
    }
}

impl Figure {
    /// The scene of the figure, rendered in `context`: first the checks that need every mark of
    /// every axes, which only the render can make (see [`resolve::check_references`]), then the
    /// pipeline.
    fn scene(&self, library: &FontLibrary, context: &'static str) -> Result<Scene> {
        resolve::check_references(self, context)?;
        pipeline::scene(self, library, context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Alpha, ColorSpec, Rgba, ScaleKind, Theme, resolve};

    /// Each series' color in panel and mark order, with label slots shared across the figure.
    fn resolved_colors(figure: &Figure) -> Vec<Rgba> {
        figure
            .panels()
            .zip(resolve::resolve_figure_marks(figure))
            .flat_map(|(axes, marks)| {
                marks
                    .into_iter()
                    .flat_map(|resolved| resolved.keys(&resolve::Limits::of(axes)))
                    .map(|key| key.color)
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn figure() -> Figure {
        Figure::new(6.0, 4.0, Theme::pigment()).unwrap()
    }

    fn nums(v: &[f64]) -> AxisData {
        AxisData::Numbers(v.to_vec())
    }

    #[test]
    fn labels_that_cannot_fit_fail_every_render_in_its_context() {
        let mut f = Figure::new(6.0, 4.0, Theme::midnight()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .xlim(0.0, 1.0)
            .unwrap()
            .xticks_labeled(&[0.0, 1.0], vec!["0".into(), "W".repeat(60)])
            .unwrap();
        let message = format!(
            "x tick label \"{}… is 306 pt wide; at most 216 pt fits this figure.",
            "W".repeat(38)
        );
        for (context, err) in [
            (
                "Figure.to_scene()",
                f.to_scene(FontLibrary::global()).unwrap_err(),
            ),
            ("Figure.to_png()", f.to_png(72).unwrap_err()),
            ("Figure.to_svg()", f.to_svg(SvgText::Outlines).unwrap_err()),
        ] {
            assert_eq!(err.to_string(), format!("{context}: {message}"));
            assert!(matches!(
                err,
                crate::Error::Style {
                    parameter: None,
                    ..
                }
            ));
            assert_eq!(err.hint(), Some("shorten the labels or widen the figure."));
        }
        // An out-of-range dpi is reported first.
        assert_eq!(f.to_png(1).unwrap_err().parameter(), Some("dpi"));
    }

    #[test]
    fn a_figure_too_short_for_the_plot_fails_every_render_in_its_context() {
        // 0.3 in is 21.6 pt; without tick labels, the paddings, the title line, the gap under
        // the header and the plot need 12 + 1.25 × 14.5 + 8 + 1 + 12 = 51.125 pt.
        let mut f = Figure::new(1.0, 0.3, Theme::midnight()).unwrap();
        f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .xticks(&[])
            .unwrap()
            .yticks(&[])
            .unwrap()
            .title("Load");
        let message = "the figure is 21 pt tall; at least 52 pt is needed for the plot area.";
        for (context, err) in [
            (
                "Figure.to_scene()",
                f.to_scene(FontLibrary::global()).unwrap_err(),
            ),
            ("Figure.to_png()", f.to_png(72).unwrap_err()),
            ("Figure.to_svg()", f.to_svg(SvgText::Outlines).unwrap_err()),
        ] {
            assert_eq!(err.to_string(), format!("{context}: {message}"));
            assert!(matches!(
                err,
                crate::Error::Style {
                    parameter: None,
                    ..
                }
            ));
            assert_eq!(
                err.hint(),
                Some("make the figure taller, or remove the title.")
            );
        }
        // An out-of-range dpi is reported first.
        assert_eq!(f.to_png(1).unwrap_err().parameter(), Some("dpi"));
    }

    #[test]
    fn auto_colors_follow_series_order() {
        let mut f = figure();
        let palette = f.theme().palette().to_vec();
        let fixed = Rgba::opaque(1, 2, 3);
        let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        let specs = [
            ColorSpec::AUTO,
            ColorSpec::Fixed(fixed),
            ColorSpec::AUTO,
            ColorSpec::Palette {
                index: 5,
                alpha: Alpha::OPAQUE,
            },
            ColorSpec::AUTO,
        ];
        for color in specs {
            a.line(
                nums(&[0.0]),
                &[1.0],
                LineOptions {
                    color,
                    ..LineOptions::default()
                },
            )
            .unwrap();
        }
        assert_eq!(
            resolved_colors(&f),
            vec![palette[0], fixed, palette[1], palette[5], palette[2]]
        );
    }

    #[test]
    fn auto_colors_count_lines_and_scatters_together() {
        let mut f = figure();
        let palette = f.theme().palette().to_vec();
        let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        a.scatter(nums(&[0.0]), &[1.0], ScatterOptions::default())
            .unwrap()
            .line(nums(&[0.0]), &[1.0], LineOptions::default())
            .unwrap();
        assert_eq!(resolved_colors(&f), vec![palette[0], palette[1]]);
        assert!(resolved_colors(&figure()).is_empty());
    }

    #[test]
    fn alpha_on_auto_is_applied() {
        let mut f = figure();
        let first = f.theme().palette()[0];
        let alpha = Alpha::new(0.5).unwrap();
        let mut a = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        a.line(
            nums(&[0.0]),
            &[1.0],
            LineOptions {
                color: ColorSpec::AUTO.with_alpha(alpha),
                ..LineOptions::default()
            },
        )
        .unwrap();
        assert_eq!(resolved_colors(&f), vec![first.with_alpha(alpha)]);
        assert_eq!(resolved_colors(&f)[0].a, alpha.to_u8());
    }

    /// A figure of category axes holding a scatter at `Q1` and `Q2` and a vertical line at `at`.
    fn category_line(at: &str) -> Figure {
        let mut f = figure();
        f.add_axes(ScaleKind::Category, ScaleKind::Linear)
            .unwrap()
            .scatter(
                AxisData::Categories(vec!["Q1".into(), "Q2".into()]),
                &[1.0, 2.0],
                crate::model::ScatterOptions::default(),
            )
            .unwrap()
            .vline(
                crate::AxisValue::Category(at.into()),
                crate::RuleOptions::default(),
            )
            .unwrap();
        f
    }

    #[test]
    fn unknown_category_fails_at_render() {
        let f = category_line("Q5");
        // The call that added the line succeeded; the three renders each fail in their own
        // context, with the name quoted and no parameter.
        let text = "x is \"Q5\"; no mark on this axes has that category.";
        let lib = FontLibrary::global();
        let errors = [
            ("Figure.to_scene()", f.to_scene(lib).unwrap_err()),
            ("Figure.to_png()", f.to_png(72).unwrap_err()),
            (
                "Figure.to_svg()",
                f.to_svg(crate::SvgText::Editable).unwrap_err(),
            ),
        ];
        for (context, err) in errors {
            assert!(matches!(err, crate::Error::Data { .. }), "{err:?}");
            assert_eq!(err.to_string(), format!("{context}: {text}"));
            assert_eq!(err.parameter(), None);
            assert_eq!(err.hint(), Some("draw a mark with that category first."));
        }
        // A known category renders; so does a name another mark brings later.
        assert!(category_line("Q2").to_scene(lib).is_ok());
        let mut f = category_line("Q5");
        f.single_axes_mut()
            .scatter(
                AxisData::Categories(vec!["Q5".into()]),
                &[3.0],
                crate::model::ScatterOptions::default(),
            )
            .unwrap();
        assert!(f.to_scene(lib).is_ok());
        // Rendering is repeatable: the failure leaves the figure as it was.
        let f = category_line("Q5");
        let before = f.clone();
        assert!(f.to_scene(lib).is_err());
        assert_eq!(f, before);
    }

    #[test]
    fn unknown_category_on_a_grid_names_its_cell() {
        let mut f = Figure::with_grid(12.0, 4.0, Theme::pigment(), crate::Grid::new(1, 2).unwrap())
            .unwrap();
        let at = |col| crate::Placement {
            row: GridIndex::At(0),
            col: GridIndex::At(col),
            ..crate::Placement::default()
        };
        f.add_axes_at(at(0), ScaleKind::Linear, ScaleKind::Linear)
            .unwrap();
        f.add_axes_at(at(1), ScaleKind::Category, ScaleKind::Linear)
            .unwrap()
            .vline(
                crate::AxisValue::Category("Q5".into()),
                crate::RuleOptions::default(),
            )
            .unwrap();
        let err = f.to_scene(FontLibrary::global()).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Figure.to_scene(): row 0, col 1: x is \"Q5\"; no mark on this axes has that category."
        );
    }
}
