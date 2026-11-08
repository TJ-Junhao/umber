//! Contours on triangle meshes in every theme: labelled lines over scattered points; filled
//! bands with a colorbar on an annulus with a hole, on equal aspect; bands and lines with NaN
//! points cutting holes; given triangles on a log x axis; and integer values whose levels fall
//! exactly on points, edges and whole triangles.

use umber_core::{
    AxisData, ColorbarOptions, ContourLevels, ContourOptions, ContourStyle, Figure, FontLibrary,
    LogBase, ScaleKind, ScatterOptions, Theme, Triangles, delaunay,
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

/// `count` scattered points over `width` by `height` from the origin, with eight points along
/// each side from its first corner, so that the triangles along the sides stay short.
fn scattered(count: usize, seed: u64, (width, height): (f64, f64)) -> (Vec<f64>, Vec<f64>) {
    let (u, v) = (uniform(count, seed), uniform(count, seed + 1));
    let (mut x, mut y) = (Vec::new(), Vec::new());
    for k in 0..8_u32 {
        let t = f64::from(k) / 8.0;
        for (px, py) in [
            (t * width, 0.0),
            (width, t * height),
            ((1.0 - t) * width, height),
            (0.0, (1.0 - t) * height),
        ] {
            x.push(px);
            y.push(py);
        }
    }
    x.extend(u.iter().map(|u| u * width));
    y.extend(v.iter().map(|v| v * height));
    (x, y)
}

/// (a) Lines with level labels over scattered points joined by their Delaunay triangles, with
/// the points on top.
fn labelled(theme: Theme) -> Figure {
    let (x, y) = scattered(220, 41, (6.0, 4.0));
    let z: Vec<f64> = x
        .iter()
        .zip(&y)
        .map(|(&x, &y)| {
            let (dx, dy) = (x - 2.2, y - 2.4);
            12.0 * (-(dx * dx) / 3.0 - dy * dy / 2.0).exp() + 1.5 * x - 0.8 * y
        })
        .collect();
    let mut figure = Figure::new(6.0, 4.0, theme).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Pressure at weather stations")
        .xlabel("East (km)")
        .ylabel("North (km)")
        .tricontour(
            &x,
            &y,
            Triangles::Auto,
            &z,
            ContourOptions {
                annotate: true,
                ..ContourOptions::default()
            },
        )
        .unwrap()
        .scatter(
            AxisData::Numbers(x.clone()),
            &y,
            ScatterOptions {
                label: "Stations".into(),
                ..ScatterOptions::default()
            },
        )
        .unwrap();
    figure
}

/// Points in the annulus between radius 0.5 and 1: rings of points on both circles, and
/// scattered points between them; and their Delaunay triangles whose centres lie outside the
/// inner circle.
fn annulus() -> (Vec<f64>, Vec<f64>, Vec<[i64; 3]>) {
    let mut points: Vec<(f64, f64)> = Vec::new();
    for (count, radius) in [(24_u32, 0.5), (48, 1.0)] {
        for k in 0..count {
            let angle = std::f64::consts::TAU * f64::from(k) / f64::from(count);
            points.push((radius * angle.cos(), radius * angle.sin()));
        }
    }
    let (u, v) = (uniform(600, 51), uniform(600, 52));
    points.extend(
        u.iter()
            .zip(&v)
            .map(|(&u, &v)| (2.0 * u - 1.0, 2.0 * v - 1.0))
            .filter(|&(x, y)| (0.54..0.96).contains(&x.hypot(y)))
            .take(260),
    );
    let (x, y): (Vec<f64>, Vec<f64>) = points.into_iter().unzip();
    let triangles = delaunay(&x, &y)
        .unwrap()
        .into_iter()
        .filter(|row| {
            let [a, b, c] = row.map(|i| usize::try_from(i).unwrap());
            ((x[a] + x[b] + x[c]) / 3.0).hypot((y[a] + y[b] + y[c]) / 3.0) > 0.5
        })
        .collect();
    (x, y, triangles)
}

/// (b) Filled bands with a colorbar on an annulus with a hole, on equal aspect.
fn holed(theme: Theme) -> Figure {
    let (x, y, triangles) = annulus();
    let z: Vec<f64> = x
        .iter()
        .zip(&y)
        .map(|(&x, &y)| {
            let (radius, angle) = (x.hypot(y), y.atan2(x));
            300.0 + 40.0 * (1.0 - radius) + 12.0 * (2.0 * angle).cos() * radius
        })
        .collect();
    let mut figure = Figure::new(5.0, 4.5, theme).unwrap();
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title("Temperature in a pipe wall")
        .tricontour(
            &x,
            &y,
            Triangles::Given(&triangles),
            &z,
            ContourOptions {
                style: ContourStyle::Filled,
                ..ContourOptions::default()
            },
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

/// (c) Bands and lines with NaN at a few points, which cut the triangles around them out.
fn missing(theme: Theme) -> Figure {
    let (x, y) = scattered(260, 61, (5.0, 4.0));
    let z: Vec<f64> = x
        .iter()
        .zip(&y)
        .enumerate()
        .map(|(i, (&x, &y))| {
            if i % 37 == 9 || (x - 3.4).hypot(y - 1.3) < 0.45 {
                f64::NAN
            } else {
                (1.3 * x).sin() * (0.9 * y).cos() + 0.15 * x
            }
        })
        .collect();
    let mut figure = Figure::new(6.0, 4.0, theme).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title("Readings with gaps")
        .tricontour(
            &x,
            &y,
            Triangles::Auto,
            &z,
            ContourOptions {
                style: ContourStyle::Both,
                annotate: true,
                center: Some(0.0),
                ..ContourOptions::default()
            },
        )
        .unwrap()
        .colorbar(ColorbarOptions::default())
        .unwrap();
    figure
}

/// A structured mesh over `x` and `y`, each quad split along the diagonal that alternates from
/// quad to quad: the points and the triangles.
fn structured(x: &[f64], y: &[f64]) -> (Vec<f64>, Vec<f64>, Vec<[i64; 3]>) {
    let columns = x.len();
    let (px, py): (Vec<f64>, Vec<f64>) = y
        .iter()
        .flat_map(|&y| x.iter().map(move |&x| (x, y)))
        .unzip();
    let at = |i: usize, j: usize| i64::try_from(j * columns + i).unwrap();
    let mut triangles = Vec::new();
    for j in 0..y.len() - 1 {
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

/// (d) Given triangles on a log x axis, the lines following the edges as drawn.
fn log_x(theme: Theme) -> Figure {
    let x = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0];
    let y = [0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0];
    let (px, py, triangles) = structured(&x, &y);
    let z: Vec<f64> = px
        .iter()
        .zip(&py)
        .map(|(&x, &y)| x / 250.0 + (3.0 - y) * x.log10() / 3.0)
        .collect();
    let mut figure = Figure::new(7.0, 4.0, theme).unwrap();
    figure
        .add_axes(ScaleKind::Log { base: LogBase::TEN }, ScaleKind::Linear)
        .unwrap()
        .title("Given triangles on a log axis")
        .xlabel("Frequency (Hz)")
        .ylabel("Depth (km)")
        .tricontour(
            &px,
            &py,
            Triangles::Given(&triangles),
            &z,
            ContourOptions {
                style: ContourStyle::Both,
                levels: ContourLevels::Count(6),
                ..ContourOptions::default()
            },
        )
        .unwrap()
        .colorbar(ColorbarOptions::default())
        .unwrap();
    figure
}

/// (e) Integer values on a structured mesh, with plateaus, ridges and single points exactly on
/// the levels.
fn integers(theme: Theme) -> Figure {
    let axis: Vec<f64> = (0..=10).map(f64::from).collect();
    let (x, y, triangles) = structured(&axis, &axis[..9]);
    let z: Vec<f64> = x
        .iter()
        .zip(&y)
        .map(|(&x, &y)| {
            let ring = (x - 5.0).abs().max((y - 4.0).abs());
            let level = (4.0 - ring).max(0.0);
            // A ridge on the level 2 along x = 8, and a single point on the level 1.
            if x == 8.0 && (2.0..=6.0).contains(&y) {
                2.0
            } else if (x, y) == (1.0, 7.0) {
                1.0
            } else {
                level
            }
        })
        .collect();
    let mut figure = Figure::new(6.0, 4.5, theme).unwrap();
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title("Counts on a mesh")
        .tricontour(
            &x,
            &y,
            Triangles::Given(&triangles),
            &z,
            ContourOptions {
                style: ContourStyle::Both,
                levels: ContourLevels::Values(vec![1.0, 2.0, 3.0]),
                annotate: true,
                ..ContourOptions::default()
            },
        )
        .unwrap()
        .colorbar(ColorbarOptions::default())
        .unwrap();
    axes.equal_aspect().unwrap();
    figure
}

/// Emits the five figures in every theme.
pub fn cases(lib: &FontLibrary, sink: &mut impl FnMut(Output)) {
    for (name, theme) in themes() {
        for (case, figure) in [
            ("labelled-lines", labelled(theme.clone())),
            ("holed-filled", holed(theme.clone())),
            ("nan-both", missing(theme.clone())),
            ("log-given", log_x(theme.clone())),
            ("integer-levels", integers(theme.clone())),
        ] {
            emit_scene(
                &format!("tricontour-{case}-{name}"),
                &figure.to_scene(lib).unwrap(),
                sink,
            );
        }
    }
}
