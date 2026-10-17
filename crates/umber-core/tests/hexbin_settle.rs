//! Automatic hexbin limits settle for dense normal clouds with sparse tails, heavy-tailed
//! samples, far outliers, random finite clouds and several hexbins on one axes.

use proptest::prelude::*;
use umber_core::{
    Figure, FontLibrary, Grid, GridIndex, HexbinOptions, Placement, ScaleKind, Share, Theme,
};

/// A seeded SplitMix64 stream of uniform values in `(0, 1)`.
struct Uniform(u64);

impl Uniform {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        ((z >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    /// A standard normal value (Box-Muller).
    fn normal(&mut self) -> f64 {
        let (u, v) = (self.next(), self.next());
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }

    /// A Student-t value with 2 degrees of freedom.
    fn student_t2(&mut self) -> f64 {
        self.normal() / (-self.next().ln()).sqrt()
    }
}

/// `n` points of `y = 0.6 x + noise` with standard normal `x` and normal noise of scale 0.7.
fn correlated_normal(seed: u64, n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut rng = Uniform(seed);
    let x: Vec<f64> = (0..n).map(|_| rng.normal()).collect();
    let y = x.iter().map(|&x| 0.6 * x + 0.7 * rng.normal()).collect();
    (x, y)
}

/// One hexbin of `x` and `y` on linear axes of a `size` figure, laid out.
fn render(x: &[f64], y: &[f64], gridsize: usize, size: (f64, f64)) -> umber_core::Result<()> {
    let mut f = Figure::new(size.0, size.1, Theme::pigment())?;
    f.add_axes(ScaleKind::Linear, ScaleKind::Linear)?.hexbin(
        x,
        y,
        HexbinOptions {
            gridsize,
            ..HexbinOptions::default()
        },
    )?;
    f.to_scene(FontLibrary::global()).map(drop)
}

/// The cases of `cases` that do not lay out, each with its error.
fn failures<'a>(
    cases: impl IntoIterator<Item = (String, &'a [f64], &'a [f64], usize, (f64, f64))>,
) -> Vec<String> {
    cases
        .into_iter()
        .filter_map(|(name, x, y, gridsize, size)| {
            render(x, y, gridsize, size)
                .err()
                .map(|error| format!("{name}, gridsize {gridsize}, {size:?}: {error}"))
        })
        .collect()
}

const SIZES: [(f64, f64); 3] = [(6.0, 4.0), (4.0, 6.0), (8.0, 3.0)];

/// Asserts that correlated normal clouds of each size in `sizes`, for three seeds, lay out at
/// each of `gridsizes` and every figure shape.
fn assert_normal_clouds_settle(sizes: &[usize], gridsizes: &'static [usize]) {
    let mut cases = Vec::new();
    for seed in 0..3 {
        for &n in sizes {
            cases.push((seed, n, correlated_normal(seed, n)));
        }
    }
    let failed = failures(cases.iter().flat_map(|(seed, n, (x, y))| {
        gridsizes.iter().flat_map(move |&gridsize| {
            let name = format!("seed {seed}, n {n}");
            SIZES.map(|size| (name.clone(), &x[..], &y[..], gridsize, size))
        })
    }));
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn dense_normal_clouds_settle() {
    assert_normal_clouds_settle(&[1_000, 10_000, 100_000], &[10, 30, 50, 100]);
}

#[test]
fn million_point_normal_clouds_settle() {
    assert_normal_clouds_settle(&[1_000_000], &[10, 50]);
}

#[test]
fn heavy_tailed_samples_settle() {
    let mut cases = Vec::new();
    for seed in 0..4 {
        for n in [1_000, 100_000] {
            let mut rng = Uniform(seed);
            let x: Vec<f64> = (0..n).map(|_| rng.student_t2()).collect();
            let y: Vec<f64> = (0..n).map(|_| rng.student_t2()).collect();
            cases.push((seed, n, (x, y)));
        }
    }
    let failed = failures(cases.iter().flat_map(|(seed, n, (x, y))| {
        [10, 30, 50, 100].into_iter().flat_map(move |gridsize| {
            SIZES.map(|size| {
                (
                    format!("t2 seed {seed}, n {n}"),
                    &x[..],
                    &y[..],
                    gridsize,
                    size,
                )
            })
        })
    }));
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn far_outliers_on_uniform_data_settle() {
    let mut cases = Vec::new();
    for (seed, outliers) in [
        (0, vec![(1e3, 0.5)]),
        (1, vec![(-40.0, 60.0), (55.0, -35.0)]),
        (2, vec![(0.5, 1e4), (-1e4, 0.5), (3e3, 3e3)]),
        (3, vec![(7.0, 7.0), (-9.0, 8.0), (12.0, -11.0)]),
    ] {
        let mut rng = Uniform(seed);
        let (mut x, mut y): (Vec<f64>, Vec<f64>) =
            (0..10_000).map(|_| (rng.next(), rng.next())).unzip();
        for (ox, oy) in outliers {
            x.push(ox);
            y.push(oy);
        }
        cases.push((seed, (x, y)));
    }
    let failed = failures(cases.iter().flat_map(|(seed, (x, y))| {
        [10, 30, 50, 100].into_iter().flat_map(move |gridsize| {
            SIZES.map(|size| (format!("outliers {seed}"), &x[..], &y[..], gridsize, size))
        })
    }));
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

/// A point of a random cloud: mostly a tight bulk, sometimes far away.
fn point() -> impl Strategy<Value = (f64, f64)> {
    prop_oneof![
        8 => (-1.0f64..1.0, -1.0f64..1.0),
        1 => (-1e3f64..1e3, -1e3f64..1e3),
        1 => (-30.0f64..30.0, -30.0f64..30.0),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn random_finite_hexbins_settle(
        points in prop::collection::vec(point(), 1..120),
        scale in prop_oneof![Just(1.0), 1e-3f64..1e3],
        gridsize in 1usize..=120,
        width in 3.0f64..10.0,
        height in 2.5f64..8.0,
        panels in 1i64..=2,
    ) {
        let (x, y): (Vec<f64>, Vec<f64>) =
            points.iter().map(|&(x, y)| (x * scale, y)).unzip();
        let grid = Grid::new(1, panels).unwrap().with_share_x(Share::All);
        let mut f = Figure::with_grid(width * panels as f64, height, Theme::pigment(), grid).unwrap();
        for col in 0..panels {
            let placement = Placement { col: GridIndex::At(col), ..Placement::default() };
            f.add_axes_at(placement, ScaleKind::Linear, ScaleKind::Linear).unwrap().hexbin(
                &x,
                &y,
                HexbinOptions { gridsize, ..HexbinOptions::default() },
            ).unwrap();
        }
        let laid_out = f.to_scene(FontLibrary::global());
        prop_assert!(laid_out.is_ok(), "{:?}", laid_out.err());
    }

    #[test]
    fn several_random_hexbins_on_one_axes_settle(
        points in prop::collection::vec(point(), 1..8),
        others in prop::collection::vec(
            (0.01f64..100.0, -5.0f64..5.0, 0.01f64..100.0, -5.0f64..5.0, 1usize..=120),
            1..=2,
        ),
        gridsize in 1usize..=120,
        width in 3.0f64..10.0,
        height in 2.5f64..8.0,
    ) {
        let (x, y): (Vec<f64>, Vec<f64>) = points.into_iter().unzip();
        let mut f = Figure::new(width, height, Theme::pigment()).unwrap();
        let mut axes = f.add_axes(ScaleKind::Linear, ScaleKind::Linear).unwrap();
        axes.hexbin(&x, &y, HexbinOptions { gridsize, ..HexbinOptions::default() }).unwrap();
        for (sx, dx, sy, dy, gridsize) in others {
            let x: Vec<f64> = x.iter().map(|x| sx * x + dx).collect();
            let y: Vec<f64> = y.iter().map(|y| sy * y + dy).collect();
            axes.hexbin(&x, &y, HexbinOptions { gridsize, ..HexbinOptions::default() }).unwrap();
        }
        let laid_out = f.to_scene(FontLibrary::global());
        prop_assert!(laid_out.is_ok(), "{:?}", laid_out.err());
    }
}
