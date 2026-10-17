//! Explicit baseline visibility changes only the baseline and its coincident gridline.

use super::*;
use crate::draw::chrome::baseline;
use crate::pipeline::plan_figure;
use crate::{BaselineVisibility, Error, Grid, GridIndex, Placement, Share};

fn figure(kind: &str) -> Figure {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let x = if kind == "bar" {
        ScaleKind::Category
    } else {
        ScaleKind::Linear
    };
    let mut ax = f.add_axes(x, ScaleKind::Linear).unwrap();
    match kind {
        "line" => {
            ax.line(
                AxisData::Numbers(vec![1.0, 2.0]),
                &[1.0, 2.0],
                Default::default(),
            )
            .unwrap();
        }
        "bar" => {
            ax.bar(
                vec!["A".into(), "B".into()],
                &[1.0, 2.0],
                Default::default(),
            )
            .unwrap();
        }
        "mesh" => {
            ax.mesh(
                &[0.0, 1.0, 2.0],
                &[-1.0, 0.0, 1.0],
                &[1.0, 2.0, 3.0, 4.0],
                Default::default(),
            )
            .unwrap();
        }
        "empty" => {}
        _ => unreachable!(),
    }
    f
}

#[test]
fn baseline_values_replace_and_auto_restores_without_changing_geometry() {
    for kind in ["line", "bar", "mesh", "empty"] {
        let mut f = figure(kind);
        let before = f.to_scene(FontLibrary::global()).unwrap();
        let (resolved, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
        for setting in [
            BaselineVisibility::Shown,
            BaselineVisibility::Hidden,
            BaselineVisibility::Shown,
            BaselineVisibility::Auto,
        ] {
            f.single_axes_mut().baseline(setting).unwrap();
            assert_eq!(f.single_axes().baseline_visibility(), setting);
            let (after, placed) = plan_figure(&f, (0, 0), FontLibrary::global());
            assert_eq!(after, resolved, "{kind}: {setting:?}");
            assert_eq!(placed.plot, layout.plot);
            assert_eq!(placed.baseline, layout.baseline);
            let scene = f.to_scene(FontLibrary::global()).unwrap();
            let expected = match setting {
                BaselineVisibility::Shown => true,
                BaselineVisibility::Hidden => false,
                BaselineVisibility::Auto => kind == "bar",
            };
            assert_eq!(
                scene.items.contains(&baseline(f.theme(), &placed)),
                expected
            );
            assert_eq!(mark_items(&scene), mark_items(&before));
        }
        assert_eq!(f.to_scene(FontLibrary::global()).unwrap(), before);
    }
}

#[test]
fn baseline_switch_replaces_only_the_gridline_at_zero_or_the_lower_edge() {
    for kind in ["line", "bar", "mesh", "empty"] {
        for (low, high) in [(-2.0, 2.0), (1.0, 3.0)] {
            let mut f = figure(kind);
            f.single_axes_mut()
                .ylim(low, high)
                .unwrap()
                .yticks(&[low, (low + high) / 2.0, high])
                .unwrap();
            for setting in [
                BaselineVisibility::Shown,
                BaselineVisibility::Hidden,
                BaselineVisibility::Auto,
            ] {
                f.single_axes_mut().baseline(setting).unwrap();
                let (_, layout) = plan_figure(&f, (0, 0), FontLibrary::global());
                let scene = f.to_scene(FontLibrary::global()).unwrap();
                let shown = setting == BaselineVisibility::Shown
                    || (setting == BaselineVisibility::Auto && kind == "bar");
                assert_eq!(scene.items.contains(&baseline(f.theme(), &layout)), shown);
                assert_eq!(layout.y_grid.contains(&layout.baseline.at), !shown);
                assert_eq!(layout.y_grid.len(), if shown { 2 } else { 3 });
            }
        }
    }
}

#[test]
fn baseline_shown_on_two_categories_is_an_atomic_usage_error() {
    let mut f = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    f.add_axes(ScaleKind::Category, ScaleKind::Category)
        .unwrap();
    for setting in [BaselineVisibility::Hidden, BaselineVisibility::Auto] {
        f.single_axes_mut().baseline(setting).unwrap();
        let before = f.clone();
        let err = f
            .single_axes_mut()
            .baseline(BaselineVisibility::Shown)
            .unwrap_err();
        assert!(matches!(err, Error::Usage { .. }));
        assert_eq!(
            err.to_string(),
            "Axes.baseline(): these axes have two category axes and no value axis; a baseline cannot be shown."
        );
        assert_eq!(err.parameter(), Some("shown"));
        assert_eq!(
            err.hint(),
            Some(
                "use shown=False or shown=ub.AUTO, or create axes with a numeric or time value axis."
            )
        );
        assert_eq!(f, before);
    }
}

#[test]
fn baseline_setting_is_local_to_shared_panels_and_cloned() {
    let grid = Grid::new(1, 2)
        .unwrap()
        .with_share_x(Share::All)
        .with_share_y(Share::All);
    let mut f = Figure::with_grid(10.0, 4.0, Theme::pigment(), grid).unwrap();
    let left = Placement {
        col: GridIndex::At(0),
        ..Placement::default()
    };
    f.add_axes_at(left, ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .baseline(BaselineVisibility::Shown)
        .unwrap();
    let right = Placement {
        col: GridIndex::At(1),
        ..Placement::default()
    };
    f.add_axes_at(right, ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    for setting in [
        BaselineVisibility::Shown,
        BaselineVisibility::Hidden,
        BaselineVisibility::Auto,
    ] {
        f.axes_at_mut(GridIndex::At(0), GridIndex::At(0))
            .unwrap()
            .baseline(setting)
            .unwrap();
        let mut cloned = f.clone();
        assert_eq!(cloned, f);
        assert_eq!(
            cloned.to_scene(FontLibrary::global()).unwrap(),
            f.to_scene(FontLibrary::global()).unwrap()
        );
        assert_eq!(
            cloned
                .axes_at_mut(GridIndex::At(0), GridIndex::At(1))
                .unwrap()
                .baseline_visibility(),
            BaselineVisibility::Auto
        );
        cloned
            .axes_at_mut(GridIndex::At(0), GridIndex::At(1))
            .unwrap()
            .baseline(BaselineVisibility::Hidden)
            .unwrap();
        assert_eq!(
            f.axes_at_mut(GridIndex::At(0), GridIndex::At(1))
                .unwrap()
                .baseline_visibility(),
            BaselineVisibility::Auto
        );
    }
}
