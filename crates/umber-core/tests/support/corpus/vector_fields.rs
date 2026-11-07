//! Vector fields in every theme: a grid quiver with a key, scattered points with `ColorBy` and a
//! colorbar, the data direction on a non-square plot, barbs on a time-height section and a
//! quiver on equal aspect.

use umber_core::{
    ArrowKey, ArrowLength, AxisData, BarbOptions, ColorBy, ColorbarOptions, Direction,
    FieldVectors, Figure, FontLibrary, Instant, PointColor, QuiverOptions, ScaleKind, Theme,
};

use super::super::{Output, emit_scene, themes};

fn numbers(values: Vec<f64>) -> AxisData {
    AxisData::Numbers(values)
}

/// `count` values from `low` to `high`.
fn spread(low: f64, high: f64, count: u32) -> Vec<f64> {
    let step = (high - low) / f64::from(count - 1);
    (0..count).map(|i| low + step * f64::from(i)).collect()
}

/// The flow `(u, v)` around a vortex at the origin, on the grid of `x` and `y`.
fn vortex(x: &[f64], y: &[f64]) -> (Vec<f64>, Vec<f64>) {
    y.iter()
        .flat_map(|&y| x.iter().map(move |&x| (x, y)))
        .map(|(x, y)| {
            let square = x * x + y * y + 0.5;
            (-y / square, x / square)
        })
        .unzip()
}

/// (a) A grid quiver with a key.
fn grid(theme: Theme) -> Figure {
    let (x, y) = (spread(-3.0, 3.0, 19), spread(-2.0, 2.0, 13));
    let (u, v) = vortex(&x, &y);
    let mut figure = Figure::new(6.0, 4.0, theme).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Flow around a vortex")
        .xlabel("x (km)")
        .ylabel("y (km)")
        .quiver(
            numbers(x),
            numbers(y),
            FieldVectors::grid(u, v),
            QuiverOptions {
                key: Some(ArrowKey::new(0.5, "0.5 m/s")),
                ..QuiverOptions::default()
            },
        )
        .unwrap();
    figure
}

/// (b) Scattered points with `ColorBy` and a colorbar.
fn stations(theme: Theme) -> Figure {
    let positions: Vec<(f64, f64)> = (0..60_u32)
        .map(|i| {
            let i = f64::from(i);
            (
                (i * 0.618_033_988_75).fract() * 10.0,
                (i * 0.754_877_666_2).fract() * 6.0,
            )
        })
        .collect();
    let (x, y): (Vec<f64>, Vec<f64>) = positions.iter().copied().unzip();
    let u: Vec<f64> = y.iter().map(|y| 1.0 + 0.4 * y).collect();
    let v: Vec<f64> = x.iter().map(|x| 0.5 * (x - 5.0)).collect();
    let speed: Vec<f64> = u
        .iter()
        .zip(&v)
        .map(|(u, v)| (u * u + v * v).sqrt())
        .collect();
    let mut figure = Figure::new(6.0, 4.0, theme).unwrap();
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title("Stations")
        .quiver(
            numbers(x),
            numbers(y),
            FieldVectors::points(u, v),
            QuiverOptions {
                color: PointColor::By(ColorBy::new(speed)),
                key: Some(ArrowKey::new(2.0, "2 m/s")),
                ..QuiverOptions::default()
            },
        )
        .unwrap();
    axes.colorbar(ColorbarOptions {
        label: "Speed (m/s)".into(),
        ..ColorbarOptions::default()
    })
    .unwrap();
    figure
}

/// (c) `Direction::Data` on a non-square plot: arrows square to the contours of `x² / 100 + y²`.
fn data_direction(theme: Theme) -> Figure {
    let (x, y) = (spread(-10.0, 10.0, 11), spread(-1.0, 1.0, 7));
    let u: Vec<f64> = y.iter().flat_map(|_| x.iter().map(|x| x / 50.0)).collect();
    let v: Vec<f64> = y
        .iter()
        .flat_map(|&y| x.iter().map(move |_| 2.0 * y))
        .collect();
    let z: Vec<f64> = y
        .iter()
        .flat_map(|&y| x.iter().map(move |&x| x * x / 100.0 + y * y))
        .collect();
    let mut figure = Figure::new(7.0, 3.0, theme).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Gradient across the level lines")
        .contour(&x, &y, &z, umber_core::ContourOptions::default())
        .unwrap()
        .quiver(
            numbers(x),
            numbers(y),
            FieldVectors::grid(u, v),
            QuiverOptions {
                direction: Direction::Data,
                length: ArrowLength::PtPerUnit(10.0),
                ..QuiverOptions::default()
            },
        )
        .unwrap();
    figure
}

/// (d) Barbs on a time-height section, from calm to strong winds.
fn section(theme: Theme) -> Figure {
    let hour = 3_600_000_000;
    let times: Vec<_> = (0..10)
        .map(|i| Some(Instant::from_micros(i * 6 * hour).unwrap()))
        .collect();
    let heights = vec![250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0];
    let (mut u, mut v) = (Vec::new(), Vec::new());
    for (row, height) in heights.iter().enumerate() {
        for column in 0..10_i32 {
            let share = height / 8000.0 * f64::from(10 - (column - 5).abs()) / 10.0;
            u.push(75.0 * share + 0.5 * f64::from(column) * row as f64);
            v.push(-25.0 * share);
        }
    }
    let mut figure = Figure::new(7.0, 4.0, theme).unwrap();
    figure
        .add_axes(ScaleKind::Time, ScaleKind::Linear)
        .unwrap()
        .title("Wind over the station")
        .ylabel("Height (m)")
        .barbs(
            AxisData::Times(times),
            numbers(heights),
            FieldVectors::grid(u, v),
            BarbOptions {
                label: "Wind (kn)".into(),
                ..BarbOptions::default()
            },
        )
        .unwrap();
    figure
}

/// (e) A quiver on equal aspect.
fn equal(theme: Theme) -> Figure {
    let (x, y) = (spread(-2.0, 2.0, 11), spread(-2.0, 2.0, 11));
    let (u, v) = vortex(&x, &y);
    let mut figure = Figure::new(7.0, 4.0, theme).unwrap();
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title("Drawn to scale")
        .quiver(
            numbers(x),
            numbers(y),
            FieldVectors::grid(u, v),
            QuiverOptions {
                key: Some(ArrowKey::new(0.5, "0.5 m/s")),
                ..QuiverOptions::default()
            },
        )
        .unwrap();
    axes.equal_aspect().unwrap();
    figure
}

/// Emits the five figures in every theme.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, theme) in themes() {
        for (case, figure) in [
            ("grid", grid(theme.clone())),
            ("stations", stations(theme.clone())),
            ("data-direction", data_direction(theme.clone())),
            ("section", section(theme.clone())),
            ("equal", equal(theme.clone())),
        ] {
            emit_scene(
                &format!("vector-fields-{case}-{name}"),
                &figure.to_scene(lib).unwrap(),
                sink,
            );
        }
    }
}
