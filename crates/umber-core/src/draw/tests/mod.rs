//! Unit tests of drawing, one suite per draw module, plus the scene-inspection helpers and
//! fixtures they share. [`oracle`] is an exact clipping reference independent of the
//! production arithmetic.

mod area;
mod band;
mod bar;
mod chrome;
mod clip;
mod error_bars;
mod hist;
mod line;
mod mapping;
mod marker;
mod marks;
mod oracle;
mod rule;
mod scatter;
mod step;

use crate::geometry::{Path, PathVerb, Point};
use crate::layout::Area;
use crate::model::{AxisData, Figure, LineOptions, ScatterOptions};
use crate::render::{SvgText, render_svg};
use crate::scene::{Item, Rect, Scene, StrokeStyle, TextItem};
use crate::test_support::image::decode_rgba8;
use crate::test_support::scene::assert_scene_bounded;
use crate::{FontLibrary, Marker, Rgba, ScaleKind, Theme};

/// The chrome group of `scene`: the first of its groups.
fn chrome_items(scene: &Scene) -> &[Item] {
    match scene.items.as_slice() {
        [Item::Group { clip: None, items }, _] | [Item::Group { clip: None, items }, _, _] => items,
        other => panic!("expected the chrome, the marks and the baseline groups, got {other:?}"),
    }
}

/// The baseline group of `scene`: the last of its groups.
fn baseline_items(scene: &Scene) -> &[Item] {
    match scene.items.as_slice() {
        [_, Item::Group { clip: None, items }] | [_, _, Item::Group { clip: None, items }] => items,
        other => panic!("expected the chrome, the marks and the baseline groups, got {other:?}"),
    }
}

/// The chrome strokes of `scene`, followed by the baseline.
fn strokes(scene: &Scene) -> Vec<(&Path, &StrokeStyle)> {
    chrome_items(scene)
        .iter()
        .chain(baseline_items(scene))
        .filter_map(|item| match item {
            Item::Stroke { path, style } => Some((path, style)),
            _ => None,
        })
        .collect()
}

fn texts(scene: &Scene) -> Vec<&TextItem> {
    chrome_items(scene)
        .iter()
        .filter_map(|item| match item {
            Item::Text(t) => Some(t),
            _ => None,
        })
        .collect()
}

fn chrome_figure(theme: Theme) -> Figure {
    let mut figure = Figure::new(6.0, 4.0, theme).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Station output")
        .subtitle("Daily mean, MW")
        .xlabel("Day")
        .ylabel("MW")
        .xlim(0.0, 100.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap();
    figure
}

#[test]
fn chrome_snapshot() {
    let scene = chrome_figure(Theme::pigment())
        .to_scene(FontLibrary::global())
        .unwrap();
    assert_scene_bounded(&scene);
    // The snapshot file is kept in `src/snapshots`, under the name of this module path.
    let snapshot_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/snapshots");
    insta::with_settings!({ snapshot_path => snapshot_path }, {
        insta::assert_snapshot!(render_svg(&scene, SvgText::Editable));
    });
}

/// The marks group of `scene`: its clip and items.
fn mark_group(scene: &Scene) -> (Rect, &[Item]) {
    match scene.items.as_slice() {
        [
            Item::Group { clip: None, .. },
            Item::Group {
                clip: Some(clip),
                items,
            },
            Item::Group { clip: None, .. },
        ] => (*clip, items),
        other => panic!("expected chrome, a clipped marks group and the baseline, got {other:?}"),
    }
}

fn line_strokes(scene: &Scene) -> Vec<(&Path, &StrokeStyle)> {
    mark_group(scene)
        .1
        .iter()
        .filter_map(|item| match item {
            Item::Stroke { path, style } => Some((path, style)),
            _ => None,
        })
        .collect()
}

/// The subpaths of `path`, as lists of points.
fn subpaths(path: &Path) -> Vec<Vec<Point>> {
    let mut out: Vec<Vec<Point>> = Vec::new();
    for verb in path.verbs() {
        match *verb {
            PathVerb::MoveTo(p) => out.push(vec![p]),
            PathVerb::LineTo(p) => out.last_mut().unwrap().push(p),
            PathVerb::Close => {}
            other => panic!("unexpected verb {other:?}"),
        }
    }
    out
}

fn nums(v: &[f64]) -> AxisData {
    AxisData::Numbers(v.to_vec())
}

fn plot_of(figure: &Figure) -> Area {
    crate::pipeline::plan_figure(figure, (0, 0), FontLibrary::global())
        .1
        .plot
}

fn line_figure(theme: Theme, x: &[f64], y: &[f64], options: LineOptions) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .line(nums(x), y, options)
        .unwrap();
    f
}

fn scatter_figure(theme: Theme, x: &[f64], y: &[f64], marker: Marker) -> Figure {
    let mut f = Figure::new(6.0, 4.0, theme).unwrap();
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .scatter(
            nums(x),
            y,
            ScatterOptions {
                marker,
                ..ScatterOptions::default()
            },
        )
        .unwrap()
        .xlim(0.0, 1.0)
        .unwrap()
        .ylim(0.0, 1.0)
        .unwrap();
    f
}

struct Pixels {
    width: usize,
    rgba: Vec<u8>,
}

impl Pixels {
    fn decode(png_bytes: &[u8]) -> Pixels {
        let image = decode_rgba8(png_bytes).unwrap();
        Pixels {
            width: image.width as usize,
            rgba: image.rgba,
        }
    }

    fn at(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y * self.width + x) * 4;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2]]
    }

    /// The pixel under the point `p`, in points, at `dpi`.
    fn at_point(&self, p: (f64, f64), dpi: f64) -> [u8; 3] {
        let s = dpi / 72.0;
        self.at((p.0 * s) as usize, (p.1 * s) as usize)
    }
}

fn near(a: [u8; 3], b: Rgba, tolerance: u8) -> bool {
    a.iter()
        .zip([b.r, b.g, b.b])
        .all(|(&x, y)| x.abs_diff(y) <= tolerance)
}
