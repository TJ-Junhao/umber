//! How marks are drawn: the cells of heatmaps, 2-D histograms and meshes as vector shapes or as
//! one image with a pixel per cell ([`Render`]), and scatters, lines and triangle meshes filled
//! by value as vector shapes or, in SVG and PDF output, as one image at the export's raster
//! resolution ([`MarkRender`]).

/// The fewest cells of a mark's grid (rows times columns) that [`Render::Auto`] draws as an
/// image.
pub(crate) const AUTO_RASTER_CELLS: usize = 10_000;

/// How the cells of a heatmap, a 2-D histogram or a mesh are drawn.
///
/// The cells of these marks form a grid of solid colors. Drawn as vector shapes, each cell is
/// one filled rectangle. Drawn as an image, the grid is one image with one pixel per cell,
/// stretched over the cells' extent with nearest-neighbour scaling, so it looks the same while
/// the output stays small and fast to render: a 1,000 × 1,000 heatmap is one image instead of a
/// million shapes. NaN values and empty bins are transparent pixels.
///
/// Both forms take their colors from the same color scale, and both are clipped to the plot
/// and drawn in the mark's place among the other marks. In PNG output each device pixel of an
/// image takes the average color of the cells it covers, weighted by the area it covers of
/// each, so neighbouring cells meet without a seam and cells smaller than a pixel blend. In SVG
/// output the image is an `<image>` holding a PNG, scaled without smoothing.
///
/// A mesh can be drawn as an image only when its edges are evenly spaced on screen: evenly
/// spaced numbers on a linear axis, evenly spaced logarithms on a log axis and evenly spaced
/// times on a time axis. The mesh call checks that every edge lies within 1/10,000 of a cell
/// of its place on the evenly spaced lattice from the first edge to the last (numbers may also
/// be off by two units of rounding error of the largest end edge; logarithms are measured from
/// the first edge, and times, whole microseconds, are compared exactly).
///
/// When the figure is drawn, an image is used only where it is exact: no edge of the image lies
/// more than 0.001 pt from the edge of the cells it stands for, on the final plot. Explicit
/// limits that zoom in on edges slightly off the lattice can move them further. An image also
/// covers whole cells, so a cell cut by explicit limits is drawn whole and clipped at the
/// limits, and a cut cell may reach at most 10,000 pt past them (more only with limits inside
/// a sliver of a few cells). A mesh or 2-D histogram that would break either bound is drawn as
/// vector shapes by `Auto`, and makes rendering fail with `Error::Style` with `Raster`.
///
/// # Examples
///
/// ```
/// use umber_core::{Figure, HeatmapOptions, Render, ScaleKind, Theme};
///
/// assert_eq!(Render::default(), Render::Auto);
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let (x, y) = (["a".to_owned(), "b".to_owned()], ["r".to_owned()]);
/// figure
///     .add_axes(ScaleKind::Category, ScaleKind::Category)?
///     .heatmap(&x, &y, &[1.0, 2.0], HeatmapOptions { render: Render::Raster, ..HeatmapOptions::default() })?;
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Render {
    /// An image when the mark's grid has at least 10,000 cells (rows times columns, such as
    /// 100 × 100), vector shapes otherwise. A mesh whose edges are not evenly spaced on screen,
    /// and a mark whose image would break the bounds above on the final plot, are drawn as
    /// vector shapes whatever their size.
    #[default]
    Auto,
    /// One filled rectangle per cell.
    Vector,
    /// One image with one pixel per cell. A mesh drawn this way needs evenly spaced edges,
    /// and rendering fails when the image would break the bounds above on the final plot.
    Raster,
}

impl Render {
    /// Whether a mark of `cells` cells (rows times columns of its grid) is drawn as an image,
    /// when its cells can be (`evenly_spaced`): always for [`Render::Raster`], from
    /// [`AUTO_RASTER_CELLS`] cells for [`Render::Auto`].
    pub(crate) fn draws_image(self, cells: usize, evenly_spaced: bool) -> bool {
        match self {
            Render::Vector => false,
            Render::Raster => evenly_spaced,
            Render::Auto => evenly_spaced && cells >= AUTO_RASTER_CELLS,
        }
    }
}

/// How a scatter, a line or a triangle mesh filled by value (tripcolor) is drawn in SVG and PDF
/// output: as vector shapes, the default, or as one image.
///
/// [`MarkRender::Raster`] is for marks so dense that vector files grow slow to open, such as a
/// scatter of a million points or a long noisy line. In SVG and PDF output such a mark is drawn
/// into one image at the raster resolution the export is given (`raster_dpi`, 300 in the Python
/// package), and the image takes the mark's place among the other marks. The image covers the
/// clip of the marks (the plot area, widened beside it for lines as that clip is), widened with
/// transparent pixels to a grid of quarter inches so that readers resample it in step with
/// their own pixels, and that clip cuts it exactly where it cuts vector shapes; marks are always clipped
/// to it, so no part of the mark is lost. The image is transparent where the mark draws
/// nothing, so marks below it show through, and its transparent pixels hold the color of the
/// nearest drawn pixel, so viewers that smooth it draw no dark edges. It holds what the PNG
/// backend draws for the mark at that resolution, pixel for pixel on the pixel grid of a PNG of
/// the figure. PNG output does not change: a raster mark is drawn as a vector one. Everything else stays vector: the axes, text, legends and
/// colorbars, the other marks, and the mark's own value labels and error bars. There is no
/// automatic choice: a mark is an image only when its options ask for one.
///
/// Unlike [`Render`] for grids of cells, the image does not stand for cells: its pixels sample
/// shapes, so viewers may smooth it when they scale it.
///
/// # Examples
///
/// ```
/// use umber_core::figure::{AxisData, ScatterOptions};
/// use umber_core::{Figure, MarkRender, ScaleKind, Theme};
///
/// assert_eq!(MarkRender::default(), MarkRender::Vector);
/// let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
/// let options = ScatterOptions { render: MarkRender::Raster, ..ScatterOptions::default() };
/// figure
///     .add_axes(ScaleKind::Linear, ScaleKind::Linear)?
///     .scatter(AxisData::Numbers(vec![0.0, 1.0]), &[1.0, 2.0], options)?;
/// let pdf = figure.to_pdf(300)?;
/// assert!(String::from_utf8_lossy(&pdf).contains("/Subtype /Image"));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MarkRender {
    /// Vector shapes in every output.
    #[default]
    Vector,
    /// One image in SVG and PDF output, at the export's raster resolution; vector shapes in PNG
    /// output, which is the same image.
    Raster,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_draws_an_image_from_ten_thousand_cells() {
        assert!(!Render::Auto.draws_image(99 * 101, true));
        assert!(Render::Auto.draws_image(100 * 100, true));
        assert!(!Render::Auto.draws_image(100 * 100, false));
        assert!(Render::Raster.draws_image(1, true));
        assert!(!Render::Vector.draws_image(1_000_000, true));
    }
}
