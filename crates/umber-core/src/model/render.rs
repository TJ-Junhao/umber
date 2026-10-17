//! How the cells of heatmaps, 2-D histograms and meshes are drawn: as vector shapes or as one
//! image.

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
/// times on a time axis. Edges count as evenly spaced when every step between two neighbouring
/// edges equals the first step to within one part in a million, allowing also for the
/// rounding of edges of large magnitude.
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
    /// 100 × 100), vector shapes otherwise. A mesh whose edges are not evenly spaced on screen
    /// is drawn as vector shapes whatever its size.
    #[default]
    Auto,
    /// One filled rectangle per cell.
    Vector,
    /// One image with one pixel per cell. A mesh drawn this way needs evenly spaced edges.
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
