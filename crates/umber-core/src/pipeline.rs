//! The render pipeline: from a [`Figure`] to its [`Scene`].
//!
//! Scales, ticks and the layout depend on each other: ticks depend on the plot size, and the
//! plot size depends on the tick label widths. [`plan`] settles them in two passes; [`scene`]
//! then draws the axes chrome and the marks.

use crate::layout::{self, Layout};
use crate::model::{Axes, Figure};
use crate::resolve::{self, Limits, Resolved, ResolvedMark};
use crate::scene::Scene;
use crate::{FontLibrary, Theme, draw};

/// Resolves and lays out the axes in two passes: ticks depend on the plot size, and the plot
/// size depends on the tick label widths. The first pass assumes the plot fills the figure
/// inside the padding; the second uses the plot size of the first, and is final. `marks` are
/// the axes' marks, resolved once by the caller; each pass resolves the axes from them.
pub(crate) fn plan(
    width: f64,
    height: f64,
    theme: &Theme,
    axes: &Axes,
    marks: &[ResolvedMark<'_>],
    lib: &FontLibrary,
) -> (Resolved, Layout) {
    let limits = Limits::of(axes);
    let guess = layout::initial_plot_size(width, height);
    let (tx, ty) = layout::tick_targets(guess);
    let first = resolve::resolve(marks, &limits, tx, ty);
    let first = layout::lay_out(width, height, theme, axes, &first, lib, guess.width);
    let (tx, ty) = layout::tick_targets(first.plot.size());
    let resolved = resolve::resolve(marks, &limits, tx, ty);
    let layout = layout::lay_out(width, height, theme, axes, &resolved, lib, first.plot.width);
    (resolved, layout)
}

/// [`plan`] for the axes of `figure`, at the figure's size, with its marks resolved against
/// its theme. Panics when the figure has no axes.
#[cfg(test)]
pub(crate) fn plan_figure(figure: &Figure, lib: &FontLibrary) -> (Resolved, Layout) {
    let theme = figure.theme();
    let axes = figure.axes().expect("the figure has axes");
    let marks = resolve::resolve_marks(axes.marks(), theme.palette());
    plan(
        figure.width_in() * 72.0,
        figure.height_in() * 72.0,
        theme,
        axes,
        &marks,
        lib,
    )
}

/// The scene of `figure`: `width × 72` by `height × 72` pt on the theme background, holding
/// the axes chrome as one unclipped group, followed by the visible marks, in insertion order,
/// as one group clipped to the plot area. Without axes it holds the background only. The marks
/// are resolved once, before layout, and the same records serve layout and drawing.
pub(crate) fn scene(figure: &Figure, library: &FontLibrary) -> Scene {
    let theme = figure.theme();
    let (width, height) = (figure.width_in() * 72.0, figure.height_in() * 72.0);
    let mut scene = Scene::new(width as f32, height as f32, theme.background());
    let Some(axes) = figure.axes() else {
        return scene;
    };
    let marks = resolve::resolve_marks(axes.marks(), theme.palette());
    let (resolved, layout) = plan(width, height, theme, axes, &marks, library);
    scene.push(draw::chrome(theme, &layout));
    if let Some(marks) = draw::marks(theme, &resolved, &layout, &marks) {
        scene.push(marks);
    }
    scene
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScaleKind;

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
