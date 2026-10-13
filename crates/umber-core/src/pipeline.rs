//! The render pipeline: from a [`Figure`] to its [`Scene`].
//!
//! Scales, ticks and the layout depend on each other: ticks depend on the plot size, and the
//! plot size depends on the tick label widths. [`plan`] settles them in two passes; [`scene`]
//! then draws the axes chrome, the marks, the legend and the baseline.

use crate::layout::{self, Layout, LayoutInput, MeasuredLegend};
use crate::legend::Legend;
use crate::model::{Axes, Figure};
use crate::resolve::{self, Limits, Resolved, ResolvedMark};
use crate::scene::Scene;
use crate::{FontLibrary, Result, Theme, draw};

/// Resolves and lays out the axes of `input` in two passes: ticks depend on the plot size, and
/// the plot size depends on the tick label widths. The first pass assumes the plot fills the
/// figure inside the padding; the second uses the plot size of the first, and is final.
/// `marks` are the axes' marks, resolved once by the caller; each pass resolves the axes from
/// them.
///
/// # Errors
///
/// As [`layout::lay_out`], from either pass: the second pass needs the plot size of the first.
pub(crate) fn plan(
    input: &LayoutInput<'_>,
    marks: &[ResolvedMark<'_>],
) -> Result<(Resolved, Layout)> {
    let limits = Limits::of(input.axes);
    let guess = layout::initial_plot_size(input.width, input.height);
    let (tx, ty) = layout::tick_targets(guess);
    let first = resolve::resolve(marks, &limits, tx, ty);
    let first = layout::lay_out(input, &first)?;
    let (tx, ty) = layout::tick_targets(first.plot.size());
    let resolved = resolve::resolve(marks, &limits, tx, ty);
    let layout = layout::lay_out(input, &resolved)?;
    Ok((resolved, layout))
}

/// The legend of `axes` with its resolved `marks`, its row and its direct labels, measured once
/// for a `width` pt canvas. The legend uses the style set on the axes, else the theme's.
fn measure_legend<'a>(
    width: f64,
    theme: &Theme,
    axes: &Axes,
    marks: &[ResolvedMark<'a>],
    lib: &FontLibrary,
) -> MeasuredLegend<'a> {
    let style = axes.legend_style().unwrap_or(theme.legend_style());
    let legend = Legend::new(style, marks, &Limits::of(axes));
    MeasuredLegend::measure(width, theme, &legend, lib)
}

/// [`plan`] for the axes of `figure`, at the figure's size, with its marks resolved against
/// its theme. Panics when the figure has no axes or cannot be laid out.
#[cfg(test)]
pub(crate) fn plan_figure(figure: &Figure, lib: &FontLibrary) -> (Resolved, Layout) {
    try_plan_figure(figure, lib).expect("the figure lays out")
}

/// [`plan`] for the axes of `figure`, as [`plan_figure`], with layout errors reported in
/// `"Figure.to_scene()"`. Panics when the figure has no axes.
#[cfg(test)]
pub(crate) fn try_plan_figure(figure: &Figure, lib: &FontLibrary) -> Result<(Resolved, Layout)> {
    with_layout_input(figure, lib, plan)
}

/// `f` called with the layout input of the axes of `figure`, at the figure's size, with layout
/// errors reported in `"Figure.to_scene()"`, and with the axes' marks resolved against the
/// figure's theme. Panics when the figure has no axes.
#[cfg(test)]
pub(crate) fn with_layout_input<R>(
    figure: &Figure,
    lib: &FontLibrary,
    f: impl FnOnce(&LayoutInput<'_>, &[ResolvedMark<'_>]) -> R,
) -> R {
    let theme = figure.theme();
    let axes = figure.axes().expect("the figure has axes");
    let (width, height) = (figure.width_in() * 72.0, figure.height_in() * 72.0);
    let marks = resolve::resolve_marks(axes.marks(), theme);
    let legend = measure_legend(width, theme, axes, &marks, lib);
    let input = LayoutInput {
        width,
        height,
        theme,
        axes,
        legend: &legend,
        lib,
        context: "Figure.to_scene()",
    };
    f(&input, &marks)
}

/// The scene of `figure`: `width × 72` by `height × 72` pt on the theme background, holding
/// - the axes chrome without the baseline, as one unclipped group;
/// - the visible marks, in insertion order, as one group clipped to the plot area (widened
///   horizontally for line ends, see `draw::marks_clip`);
/// - the legend, when it has row entries or direct labels, as one unclipped group;
/// - the baseline, as an unclipped group of its own.
///
/// Without axes it holds the background only. The marks are resolved and the legend is built
/// and measured once, before layout, and the same records serve layout and drawing.
///
/// # Errors
///
/// As [`plan`], reported in `context`, the API call that renders.
pub(crate) fn scene(
    figure: &Figure,
    library: &FontLibrary,
    context: &'static str,
) -> Result<Scene> {
    let theme = figure.theme();
    let (width, height) = (figure.width_in() * 72.0, figure.height_in() * 72.0);
    let mut scene = Scene::new(width as f32, height as f32, theme.background());
    let Some(axes) = figure.axes() else {
        return Ok(scene);
    };
    let marks = resolve::resolve_marks(axes.marks(), theme);
    let legend = measure_legend(width, theme, axes, &marks, library);
    let input = LayoutInput {
        width,
        height,
        theme,
        axes,
        legend: &legend,
        lib: library,
        context,
    };
    let (resolved, layout) = plan(&input, &marks)?;
    scene.push(draw::chrome(theme, &layout));
    if let Some(marks) = draw::marks(theme, &resolved, &layout, &marks) {
        scene.push(marks);
    }
    if let Some(legend) = draw::legend::legend(theme, &layout) {
        scene.push(legend);
    }
    scene.push(draw::baseline(theme, &layout));
    Ok(scene)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AxisData, LineOptions};
    use crate::scene::Item;
    use crate::{LegendStyle, ScaleKind};

    /// A figure in `theme` with a title and two labelled lines.
    fn labelled_figure(theme: Theme) -> Figure {
        let mut f = Figure::new(6.0, 4.0, theme).unwrap();
        let axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        axes.title("Station output");
        for label in ["North", "South"] {
            axes.line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                LineOptions {
                    label: label.to_owned(),
                    ..LineOptions::default()
                },
            )
            .unwrap();
        }
        f
    }

    /// The number of legend entries laid out, and the number of top-level groups in the scene:
    /// chrome, marks and baseline, plus one with a legend.
    fn legend_and_groups(f: &Figure) -> (usize, usize) {
        let lib = FontLibrary::global();
        let entries = plan_figure(f, lib).1.legend.entries().len();
        let groups = scene(f, lib, "Figure.to_scene()")
            .unwrap()
            .items
            .iter()
            .filter(|item| matches!(item, Item::Group { .. }))
            .count();
        (entries, groups)
    }

    #[test]
    fn header_theme_shows_the_legend_between_marks_and_baseline() {
        let f = labelled_figure(Theme::pigment());
        assert_eq!(legend_and_groups(&f), (2, 4));
        let s = scene(&f, FontLibrary::global(), "Figure.to_scene()").unwrap();
        // Chrome, marks, legend (unclipped), baseline.
        assert!(
            matches!(s.items.get(2), Some(Item::Group { clip: None, items }) if items.len() == 4)
        );
    }

    #[test]
    fn theme_none_hides_the_legend() {
        let theme = Theme::pigment()
            .with_legend_style(LegendStyle::None)
            .unwrap();
        assert_eq!(theme.legend_style(), LegendStyle::None);
        assert_eq!(legend_and_groups(&labelled_figure(theme)), (0, 3));
    }

    #[test]
    fn axes_none_hides_the_legend_under_a_header_theme() {
        let mut f = labelled_figure(Theme::pigment());
        f.axes_mut().unwrap().legend(LegendStyle::None);
        assert_eq!(legend_and_groups(&f), (0, 3));
        // Without a legend the figure draws as if its marks were unlabelled.
        let mut unlabelled = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        let axes = unlabelled
            .add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap();
        axes.title("Station output");
        for _ in 0..2 {
            axes.line(
                AxisData::Numbers(vec![0.0, 1.0]),
                &[1.0, 2.0],
                LineOptions::default(),
            )
            .unwrap();
        }
        let lib = FontLibrary::global();
        let scene_of = |f: &Figure| scene(f, lib, "Figure.to_scene()").unwrap();
        assert_eq!(scene_of(&f), scene_of(&unlabelled));
    }

    #[test]
    fn axes_header_shows_the_legend_under_a_none_theme() {
        let none = Theme::pigment()
            .with_legend_style(LegendStyle::None)
            .unwrap();
        let mut f = labelled_figure(none);
        f.axes_mut().unwrap().legend(LegendStyle::Header);
        assert_eq!(legend_and_groups(&f), (2, 4));
        // The same scene as under a theme whose own style is the header row.
        let scene_of = |f: &Figure| scene(f, FontLibrary::global(), "Figure.to_scene()").unwrap();
        assert_eq!(scene_of(&f), scene_of(&labelled_figure(Theme::pigment())));
    }

    #[test]
    fn axes_style_replaces_the_theme_style_and_itself() {
        // A theme with direct labels: no row entries, both lines labelled directly, in a
        // legend group.
        let mut f = labelled_figure(Theme::graphite());
        let direct = |f: &Figure| plan_figure(f, FontLibrary::global()).1.direct.len();
        assert_eq!(legend_and_groups(&f), (0, 4));
        assert_eq!(direct(&f), 2);
        // The axes ask for the header row.
        f.axes_mut().unwrap().legend(LegendStyle::Header);
        assert_eq!(legend_and_groups(&f), (2, 4));
        assert_eq!(direct(&f), 0);
        // Calling it again replaces the style.
        f.axes_mut().unwrap().legend(LegendStyle::None);
        assert_eq!(legend_and_groups(&f), (0, 3));
        assert_eq!(direct(&f), 0);
    }

    #[test]
    fn tick_targets_follow_plot_size() {
        // The second pass uses the first pass's plot size: a taller figure gets more y ticks.
        let ticks = |w: f64, h: f64| {
            let mut f = Figure::new(w, h, Theme::graphite()).unwrap();
            f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
                .unwrap()
                .ylim(0.0, 100.0)
                .unwrap()
                .xlim(0.0, 100.0)
                .unwrap();
            let (resolved, layout) = plan_figure(&f, FontLibrary::global());
            let targets = layout::tick_targets(layout.plot.size());
            (resolved.x.ticks.len(), resolved.y.ticks.len(), targets)
        };
        let (sx, sy, small) = ticks(2.0, 1.5);
        let (lx, ly, large) = ticks(12.0, 9.0);
        assert_eq!(small, (3, 3));
        assert_eq!(large, (8, 8));
        assert!(lx > sx && ly > sy, "{sx} {sy} {lx} {ly}");
    }
}
