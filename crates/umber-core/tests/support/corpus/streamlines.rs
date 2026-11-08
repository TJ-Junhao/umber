//! Streamlines in every theme: flow around a cylinder with a hole for the cylinder on equal
//! aspect, the same colored by speed with a colorbar, lines through given starts in a rotation,
//! and an unevenly spaced grid on a log x axis with a label in the legend.

use umber_core::{
    ColorBy, ColorbarOptions, Figure, FontLibrary, LogBase, PointColor, ScaleKind, StreamStarts,
    StreamplotOptions, Theme,
};

use super::super::{Output, emit_scene, themes};

/// `count` values from `low` to `high`.
fn spread(low: f64, high: f64, count: u32) -> Vec<f64> {
    let step = (high - low) / f64::from(count - 1);
    (0..count).map(|i| low + step * f64::from(i)).collect()
}

/// The field `f(x, y) = (u, v)` on the grid of `x` and `y`, row by row.
fn sampled(x: &[f64], y: &[f64], f: impl Fn(f64, f64) -> (f64, f64)) -> (Vec<f64>, Vec<f64>) {
    y.iter()
        .flat_map(|&y| x.iter().map(move |&x| (x, y)))
        .map(|(x, y)| f(x, y))
        .unzip()
}

/// A field on a grid, with its speed.
struct Field {
    x: Vec<f64>,
    y: Vec<f64>,
    u: Vec<f64>,
    v: Vec<f64>,
    speed: Vec<f64>,
}

/// The potential flow around a cylinder of radius 1 at the origin, NaN inside it.
fn cylinder() -> Field {
    let (x, y) = (spread(-3.0, 3.0, 121), spread(-2.0, 2.0, 81));
    let (u, v) = sampled(&x, &y, |x, y| {
        let square = x * x + y * y;
        if square < 1.0 {
            return (f64::NAN, f64::NAN);
        }
        let fourth = square * square;
        (1.0 - (x * x - y * y) / fourth, -2.0 * x * y / fourth)
    });
    let speed = u
        .iter()
        .zip(&v)
        .map(|(u, v)| (u * u + v * v).sqrt())
        .collect();
    Field { x, y, u, v, speed }
}

/// (a) and (b): the flow around a cylinder on equal aspect, in one color or colored by speed
/// with a colorbar.
fn around(theme: Theme, by_speed: bool) -> Figure {
    let Field { x, y, u, v, speed } = cylinder();
    let mut figure = Figure::new(7.0, 4.5, theme).unwrap();
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    let color = if by_speed {
        PointColor::By(ColorBy::new(speed))
    } else {
        PointColor::AUTO
    };
    axes.title("Flow around a cylinder")
        .xlabel("x / radius")
        .ylabel("y / radius")
        .streamplot(
            &x,
            &y,
            &u,
            &v,
            StreamplotOptions {
                color,
                ..StreamplotOptions::default()
            },
        )
        .unwrap();
    if by_speed {
        axes.colorbar(ColorbarOptions {
            label: "Speed / free stream".into(),
            ..ColorbarOptions::default()
        })
        .unwrap();
    }
    axes.equal_aspect().unwrap();
    figure
}

/// (c) Lines through given starts in a rotation: closed orbits that end after one turn.
fn rotation(theme: Theme) -> Figure {
    let (x, y) = (spread(-1.0, 1.0, 21), spread(-1.0, 1.0, 21));
    let (u, v) = sampled(&x, &y, |x, y| (-y, x));
    let radii = [0.15, 0.3, 0.45, 0.6, 0.75, 0.9];
    let mut figure = Figure::new(5.0, 5.0, theme).unwrap();
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title("Rotation")
        .streamplot(
            &x,
            &y,
            &u,
            &v,
            StreamplotOptions {
                starts: StreamStarts::At {
                    x: radii.to_vec(),
                    y: vec![0.0; radii.len()],
                },
                ..StreamplotOptions::default()
            },
        )
        .unwrap();
    axes.equal_aspect().unwrap();
    figure
}

/// (d) An unevenly spaced grid on a log x axis, with a label in the legend.
fn log_x(theme: Theme) -> Figure {
    let x = vec![1.0, 2.0, 5.0, 10.0, 30.0, 100.0, 300.0, 1000.0];
    let y = vec![0.0, 0.4, 1.2, 2.0, 3.5, 5.0];
    // Uniform along the logarithm of x, turning from down to up across it.
    let (u, v) = sampled(&x, &y, |x, y| {
        (x, 0.8 * (1.0 - 2.0 / (1.0 + x / 30.0)) * (1.0 - y / 5.0))
    });
    let mut figure = Figure::new(7.0, 4.0, theme).unwrap();
    figure
        .add_axes(ScaleKind::Log { base: LogBase::TEN }, ScaleKind::Linear)
        .unwrap()
        .title("Uneven grid on a log axis")
        .xlabel("Frequency (Hz)")
        .streamplot(
            &x,
            &y,
            &u,
            &v,
            StreamplotOptions {
                label: "Flow".into(),
                density: 0.6,
                ..StreamplotOptions::default()
            },
        )
        .unwrap();
    figure
}

/// Emits the four figures in every theme.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, theme) in themes() {
        for (case, figure) in [
            ("cylinder", around(theme.clone(), false)),
            ("cylinder-speed", around(theme.clone(), true)),
            ("rotation-starts", rotation(theme.clone())),
            ("log-uneven", log_x(theme.clone())),
        ] {
            emit_scene(
                &format!("streamlines-{case}-{name}"),
                &figure.to_scene(lib).unwrap(),
                sink,
            );
        }
    }
}
