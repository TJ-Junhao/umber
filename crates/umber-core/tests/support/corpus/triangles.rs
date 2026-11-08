//! Triangle meshes in every theme: the edges of a mesh with a hole cut out of its Delaunay
//! triangles, with the points on top; the triangles colored by values at the points, with a
//! colorbar, on equal aspect; triangles colored by their own values, some blank; and given
//! triangles on a log x axis, their edges over their colors.

use umber_core::{
    AxisData, ColorbarOptions, Figure, FontLibrary, LogBase, ScaleKind, ScatterOptions, Theme,
    TriValues, Triangles, TripcolorOptions, TriplotOptions, delaunay,
};

use super::super::{Output, emit_scene, themes};

/// `count` deterministic values from 0 to 1.
fn uniform(count: usize, seed: u64) -> Vec<f64> {
    let mut state = seed;
    (0..count)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 11) as f64 / (1_u64 << 53) as f64
        })
        .collect()
}

/// Points in the annulus between radius 0.5 and 1: rings of points on both circles, and
/// scattered points between them.
fn annulus() -> (Vec<f64>, Vec<f64>) {
    let mut points: Vec<(f64, f64)> = Vec::new();
    for (count, radius) in [(18_u32, 0.5), (36, 1.0)] {
        for k in 0..count {
            let angle = std::f64::consts::TAU * f64::from(k) / f64::from(count);
            points.push((radius * angle.cos(), radius * angle.sin()));
        }
    }
    let (u, v) = (uniform(400, 11), uniform(400, 12));
    points.extend(
        u.iter()
            .zip(&v)
            .map(|(&u, &v)| (2.0 * u - 1.0, 2.0 * v - 1.0))
            .filter(|&(x, y)| (0.56..0.95).contains(&x.hypot(y)))
            .take(140),
    );
    points.into_iter().unzip()
}

/// The Delaunay triangles of the points `(x[i], y[i])` whose centers lie outside the circle of
/// radius `hole` around the origin.
fn outside(x: &[f64], y: &[f64], hole: f64) -> Vec<[i64; 3]> {
    delaunay(x, y)
        .unwrap()
        .into_iter()
        .filter(|row| {
            let [a, b, c] = row.map(|i| usize::try_from(i).unwrap());
            let (cx, cy) = ((x[a] + x[b] + x[c]) / 3.0, (y[a] + y[b] + y[c]) / 3.0);
            cx.hypot(cy) > hole
        })
        .collect()
}

/// (a) The edges of a mesh with a hole, with the points on top.
fn holed(theme: Theme) -> Figure {
    let (x, y) = annulus();
    let triangles = outside(&x, &y, 0.5);
    let mut figure = Figure::new(5.0, 5.0, theme).unwrap();
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title("A mesh with a hole")
        .triplot(
            &x,
            &y,
            Triangles::Given(&triangles),
            TriplotOptions {
                label: "Edges".into(),
                ..TriplotOptions::default()
            },
        )
        .unwrap()
        .scatter(
            AxisData::Numbers(x.clone()),
            &y,
            ScatterOptions {
                label: "Points".into(),
                ..ScatterOptions::default()
            },
        )
        .unwrap();
    axes.equal_aspect().unwrap();
    figure
}

/// (b) The triangles colored by values at the points, with a colorbar, on equal aspect.
fn on_points(theme: Theme) -> Figure {
    let (u, v) = (uniform(260, 21), uniform(260, 22));
    let (x, y): (Vec<f64>, Vec<f64>) = u
        .iter()
        .zip(&v)
        .map(|(&u, &v)| (4.0 * u - 2.0, 3.0 * v - 1.5))
        .unzip();
    let values: Vec<f64> = x
        .iter()
        .zip(&y)
        .map(|(&x, &y)| (-(x * x) - 1.5 * y * y).exp() * (1.0 + 0.4 * x) - 0.3 * y)
        .collect();
    let mut figure = Figure::new(6.0, 4.0, theme).unwrap();
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title("Values at scattered points")
        .xlabel("x (m)")
        .ylabel("y (m)")
        .tripcolor(
            &x,
            &y,
            Triangles::Auto,
            &values,
            TripcolorOptions::default(),
        )
        .unwrap()
        .colorbar(ColorbarOptions {
            label: "Temperature (K)".into(),
            ..ColorbarOptions::default()
        })
        .unwrap();
    axes.equal_aspect().unwrap();
    figure
}

/// A structured mesh of `columns` by `rows` quads over `x` and `y`, each split along the
/// diagonal that alternates from quad to quad: the points and the triangles.
fn structured(x: &[f64], y: &[f64]) -> (Vec<f64>, Vec<f64>, Vec<[i64; 3]>) {
    let (columns, rows) = (x.len(), y.len());
    let (px, py): (Vec<f64>, Vec<f64>) = y
        .iter()
        .flat_map(|&y| x.iter().map(move |&x| (x, y)))
        .unzip();
    let at = |i: usize, j: usize| i64::try_from(j * columns + i).unwrap();
    let mut triangles = Vec::new();
    for j in 0..rows - 1 {
        for i in 0..columns - 1 {
            let corners = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
            if (i + j) % 2 == 0 {
                triangles.push([corners[0], corners[1], corners[2]]);
                triangles.push([corners[0], corners[2], corners[3]]);
            } else {
                triangles.push([corners[0], corners[1], corners[3]]);
                triangles.push([corners[1], corners[2], corners[3]]);
            }
        }
    }
    (px, py, triangles)
}

/// (c) Triangles colored by their own values, with some blank.
fn on_triangles(theme: Theme) -> Figure {
    let grid = |count: u32, high: f64| {
        (0..count)
            .map(|k| high * f64::from(k) / f64::from(count - 1))
            .collect::<Vec<f64>>()
    };
    let (x, y, triangles) = structured(&grid(13, 6.0), &grid(9, 4.0));
    let values: Vec<f64> = triangles
        .iter()
        .enumerate()
        .map(|(k, row)| {
            let [a, b, c] = row.map(|i| usize::try_from(i).unwrap());
            let (cx, cy) = ((x[a] + x[b] + x[c]) / 3.0, (y[a] + y[b] + y[c]) / 3.0);
            if k % 17 == 5 || (cx - 4.5).hypot(cy - 2.8) < 0.7 {
                f64::NAN
            } else {
                40.0 + 12.0 * (cx / 6.0) * (1.0 - cy / 8.0) + 3.0 * ((k % 5) as f64)
            }
        })
        .collect();
    let mut figure = Figure::new(6.0, 4.0, theme).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Stress per element")
        .tripcolor(
            &x,
            &y,
            Triangles::Given(&triangles),
            &values,
            TripcolorOptions {
                on: TriValues::Triangles,
                ..TripcolorOptions::default()
            },
        )
        .unwrap()
        .colorbar(ColorbarOptions {
            label: "Stress (MPa)".into(),
            ..ColorbarOptions::default()
        })
        .unwrap();
    figure
}

/// (d) Given triangles on a log x axis, their edges over their colors.
fn log_x(theme: Theme) -> Figure {
    let x = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0];
    let y = [0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0];
    let (px, py, triangles) = structured(&x, &y);
    let values: Vec<f64> = px
        .iter()
        .zip(&py)
        .map(|(&x, &y)| x.log10() * (3.0 - y) + 0.5 * y * y)
        .collect();
    let mut figure = Figure::new(7.0, 4.0, theme).unwrap();
    figure
        .add_axes(ScaleKind::Log { base: LogBase::TEN }, ScaleKind::Linear)
        .unwrap()
        .title("Given triangles on a log axis")
        .xlabel("Frequency (Hz)")
        .ylabel("Depth (km)")
        .tripcolor(
            &px,
            &py,
            Triangles::Given(&triangles),
            &values,
            TripcolorOptions::default(),
        )
        .unwrap()
        .triplot(
            &px,
            &py,
            Triangles::Given(&triangles),
            TriplotOptions {
                label: "Elements".into(),
                ..TriplotOptions::default()
            },
        )
        .unwrap()
        .colorbar(ColorbarOptions::default())
        .unwrap();
    figure
}

/// Emits the four figures in every theme.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, theme) in themes() {
        for (case, figure) in [
            ("holed-edges", holed(theme.clone())),
            ("on-points", on_points(theme.clone())),
            ("on-triangles", on_triangles(theme.clone())),
            ("log-given", log_x(theme.clone())),
        ] {
            emit_scene(
                &format!("triangles-{case}-{name}"),
                &figure.to_scene(lib).unwrap(),
                sink,
            );
        }
    }
}
