//! Render benchmarks: the spec's timing targets plus SVG, marks, legends and text-heavy cases.
//!
//! ```text
//! cargo bench -p umber-core --bench render
//! ```
//!
//! Inputs are deterministic. The numbers track regressions between two revisions measured on
//! the same machine; they are not absolute pass/fail limits. `render_png` rasterizes and
//! encodes in one call, so the encode step is timed on its own by re-encoding the decoded
//! pixels of the same figure with the encoder settings the renderer uses.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::hint::black_box;
use std::time::Duration;

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use umber_core::figure::{AxisData, HistOptions, LineOptions, ScatterOptions};
use umber_core::render::{SvgText, render_png, render_svg};
use umber_core::{
    AreaOptions, BandOptions, ColorBy, Figure, FontLibrary, Grid, GridIndex, HeatmapOptions,
    HexbinOptions, Hist2dOptions, Instant, LegendPosition, LegendStyle, Placement, PointColor,
    ScaleKind, Series, Share, Theme,
};

const DPI: u32 = 200;

/// A deterministic value in `[0, 1)` from `i`, so inputs need no random number crate.
fn unit(i: usize) -> f64 {
    let mut h = (i as u64).wrapping_add(0x9e37_79b9_7f4a_7c15);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^= h >> 31;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

fn xs(n: usize) -> Vec<f64> {
    (0..n).map(|i| i as f64 / n as f64 * 100.0).collect()
}

/// A noisy wave, so a line has many distinct pixels to draw.
fn wave(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| (i as f64 * 0.001).sin() * 10.0 + unit(i) * 2.0)
        .collect()
}

fn line_figure(n: usize) -> Figure {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .line(AxisData::Numbers(xs(n)), &wave(n), LineOptions::default())
        .unwrap();
    figure
}

/// Twenty 10k-point lines at 12x8 in; only scale sharing differs between the two cases.
fn grid_figure(share: Share) -> Figure {
    let grid = Grid::new(4, 5)
        .unwrap()
        .with_share_x(share)
        .with_share_y(share);
    let mut figure = Figure::with_grid(12.0, 8.0, Theme::pigment(), grid).unwrap();
    let x = xs(10_000);
    let y = wave(10_000);
    for row in 0..4 {
        for col in 0..5 {
            let at = Placement {
                row: GridIndex::At(row),
                col: GridIndex::At(col),
                ..Placement::default()
            };
            figure
                .add_axes_at(at, ScaleKind::Linear, ScaleKind::Linear)
                .unwrap()
                .line(AxisData::Numbers(x.clone()), &y, LineOptions::default())
                .unwrap();
        }
    }
    figure
}

fn scatter_figure(n: usize) -> Figure {
    let x: Vec<f64> = (0..n).map(|i| unit(2 * i) * 100.0).collect();
    let y: Vec<f64> = (0..n).map(|i| unit(2 * i + 1) * 50.0).collect();
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .scatter(AxisData::Numbers(x), &y, ScatterOptions::default())
        .unwrap();
    figure
}

fn mapped_scatter_figure(n: usize) -> Figure {
    let x: Vec<f64> = (0..n).map(|i| unit(2 * i) * 100.0).collect();
    let y: Vec<f64> = (0..n).map(|i| unit(2 * i + 1) * 50.0).collect();
    let values: Vec<f64> = (0..n).map(|i| unit(2 * n + i) * 2.0 - 1.0).collect();
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .scatter(
            AxisData::Numbers(x),
            &y,
            ScatterOptions {
                color: PointColor::By(ColorBy {
                    center: Some(0.0),
                    ..ColorBy::new(values)
                }),
                ..ScatterOptions::default()
            },
        )
        .unwrap();
    figure
}

fn hist_figure(n: usize) -> Figure {
    // The sum of four uniforms is bell-shaped, so the bins hold uneven counts.
    let values: Vec<f64> = (0..n)
        .map(|i| (0..4).map(|k| unit(4 * i + k)).sum::<f64>())
        .collect();
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .hist(&values, HistOptions::default())
        .unwrap();
    figure
}

/// A 1000x1000 heatmap on two category axes.
fn heatmap_figure() -> Figure {
    let names =
        |prefix: &str| -> Vec<String> { (0..1000).map(|i| format!("{prefix}{i}")).collect() };
    let values: Vec<f64> = (0..1_000_000).map(unit).collect();
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Category, ScaleKind::Category)
        .unwrap()
        .heatmap(&names("x"), &names("y"), &values, HeatmapOptions::default())
        .unwrap();
    figure
}

/// The 200-category vertical bars from the exact-output corpus.
fn category_figure() -> Figure {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let names = (0..200).map(|i| format!("c{i}")).collect();
    let values: Vec<_> = (0..200).map(|i| 1.0 + f64::from(i % 20)).collect();
    figure
        .add_axes(ScaleKind::Category, ScaleKind::Linear)
        .unwrap()
        .bar(names, &values, umber_core::BarOptions::default())
        .unwrap();
    figure
}

/// Correlated points: y follows x plus bell-shaped noise, so the bins hold uneven counts.
fn correlated(n: usize) -> (Vec<f64>, Vec<f64>) {
    let x: Vec<f64> = (0..n)
        .map(|i| (0..4).map(|k| unit(5 * i + k)).sum::<f64>())
        .collect();
    let y = x
        .iter()
        .enumerate()
        .map(|(i, v)| 0.7 * v + 0.3 * unit(5 * i + 4) * 4.0)
        .collect();
    (x, y)
}

fn hist2d_figure(n: usize) -> Figure {
    let (x, y) = correlated(n);
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .hist2d(
            &x,
            &y,
            Hist2dOptions {
                bins: (100, 100),
                ..Hist2dOptions::default()
            },
        )
        .unwrap();
    figure
}

fn hexbin_figure(n: usize) -> Figure {
    let (x, y) = correlated(n);
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .hexbin(
            &x,
            &y,
            HexbinOptions {
                gridsize: 50,
                ..HexbinOptions::default()
            },
        )
        .unwrap();
    figure
}

/// The `line_figure` wave on a time x axis, one point per second.
fn time_line_figure(n: usize) -> Figure {
    let times: Vec<Option<Instant>> = (0..n)
        .map(|i| Some(Instant::from_micros(i as i64 * 1_000_000).unwrap()))
        .collect();
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Time, ScaleKind::Linear)
        .unwrap()
        .line(AxisData::Times(times), &wave(n), LineOptions::default())
        .unwrap();
    figure
}

/// Long title and subtitle, axis labels, a legend with four series and many tick labels.
fn text_figure() -> Figure {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let mut axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title("Quarterly output across the northern and southern grids")
        .subtitle("Measured every fifteen minutes from January to December")
        .xlabel("Elapsed time since the first measurement (days)")
        .ylabel("Output per station (megawatt hours)")
        .legend(LegendStyle::Header, umber_core::SizeSpec::Auto)
        .unwrap();
    let positions: Vec<f64> = (0..24).map(f64::from).collect();
    let labels: Vec<String> = (0..24).map(|i| format!("Week {i:02} of the run")).collect();
    axes.xticks_labeled(&positions, labels).unwrap();
    for (k, name) in ["North grid", "South grid", "East grid", "West grid"]
        .into_iter()
        .enumerate()
    {
        let x: Vec<f64> = (0..200).map(|i| i as f64 / 200.0 * 23.0).collect();
        let y: Vec<f64> = (0..200)
            .map(|i| (i as f64 * 0.05 + k as f64).sin() * 5.0 + k as f64 * 3.0)
            .collect();
        axes.line(
            AxisData::Numbers(x),
            &y,
            LineOptions {
                label: name.to_owned(),
                ..LineOptions::default()
            },
        )
        .unwrap();
    }
    figure
}

/// The PNG of `figure` at [`DPI`], decoded back to 8-bit RGBA pixels.
fn pixels_of(figure: &Figure) -> (u32, u32, Vec<u8>) {
    let png = figure.to_png(i64::from(DPI)).unwrap();
    let mut reader = png::Decoder::new(std::io::Cursor::new(png))
        .read_info()
        .unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    (info.width, info.height, buf)
}

/// What `render_png` does after rasterizing, with the same encoder settings.
fn encode(width: u32, height: u32, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_deflate_compression(png::DeflateCompression::Level(3));
    encoder.set_filter(png::Filter::Adaptive);
    let per_metre = (f64::from(DPI) / 0.0254).round() as u32;
    encoder.set_pixel_dims(Some(png::PixelDimensions {
        xppu: per_metre,
        yppu: per_metre,
        unit: png::Unit::Meter,
    }));
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(data).unwrap();
    writer.finish().unwrap();
    out
}

fn benches(c: &mut Criterion) {
    let library = FontLibrary::global();
    // Touch the font library once so no case pays for loading it.
    black_box(line_figure(10).to_scene(library).unwrap());

    let mut group = c.benchmark_group("render");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(5));

    // The spec's targets: 1M-point line and 100k scatter, 6x4 in at 200 dpi, end to end
    // from a built figure.
    let line = line_figure(1_000_000);
    group.bench_function("line_1m_png", |b| {
        b.iter(|| black_box(line.to_png(i64::from(DPI)).unwrap()));
    });
    let scatter = scatter_figure(100_000);
    group.bench_function("scatter_100k_png", |b| {
        b.iter(|| black_box(scatter.to_png(i64::from(DPI)).unwrap()));
    });
    let mapped_scatter = mapped_scatter_figure(100_000);
    group.bench_function("scatter_100k_mapped_png", |b| {
        b.iter(|| black_box(mapped_scatter.to_png(i64::from(DPI)).unwrap()));
    });
    for dpi in [100, 144] {
        group.bench_function(format!("scatter_100k_png_{dpi}dpi"), |b| {
            b.iter(|| black_box(scatter.to_png(dpi).unwrap()));
        });
        group.bench_function(format!("scatter_100k_mapped_png_{dpi}dpi"), |b| {
            b.iter(|| black_box(mapped_scatter.to_png(dpi).unwrap()));
        });
    }

    // The v0.2 grid target, including the shared-axes resolve/layout repair path.
    for (name, share) in [
        ("grid_20_lines_10k_png", Share::None),
        ("grid_20_lines_10k_png_shared", Share::All),
    ] {
        let grid = grid_figure(share);
        group.bench_function(name, |b| {
            b.iter(|| black_box(grid.to_png(i64::from(DPI)).unwrap()));
        });
    }

    // End-to-end PNG complements the scene-only legend-position comparisons below.
    let mut stacked_area = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let y = wave(100_000);
    let mut axes = stacked_area
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.legend_at(
        LegendStyle::Header,
        LegendPosition::Best,
        umber_core::SizeSpec::Auto,
    )
    .unwrap();
    axes.stacked_area(
        &xs(100_000),
        (0..3)
            .map(|k| Series {
                label: format!("Layer {k}"),
                values: y.iter().map(|v| v + 12.0 + k as f64).collect(),
                ..Series::default()
            })
            .collect(),
    )
    .unwrap();
    group.bench_function("best_legend_stacked_area", |b| {
        b.iter(|| black_box(stacked_area.to_png(i64::from(DPI)).unwrap()));
    });

    // Split timings for the scatter case: build the figure, resolve and lay out and draw the
    // scene, rasterize plus encode, and encode alone.
    group.bench_function("scatter_100k_build", |b| {
        b.iter(|| black_box(scatter_figure(100_000)));
    });
    group.bench_function("scatter_100k_scene", |b| {
        b.iter(|| black_box(scatter.to_scene(library).unwrap()));
    });
    let scene = scatter.to_scene(library).unwrap();
    group.bench_function("scatter_100k_rasterize_and_encode", |b| {
        b.iter(|| black_box(render_png(&scene, DPI).unwrap()));
    });
    let (w, h, data) = pixels_of(&scatter);
    group.bench_function("scatter_100k_encode_only", |b| {
        b.iter_batched(
            || (),
            |()| black_box(encode(w, h, &data)),
            BatchSize::PerIteration,
        );
    });
    // The same split for the line case.
    group.bench_function("line_1m_scene", |b| {
        b.iter(|| black_box(line.to_scene(library).unwrap()));
    });
    let line_scene = line.to_scene(library).unwrap();
    group.bench_function("line_1m_rasterize_and_encode", |b| {
        b.iter(|| black_box(render_png(&line_scene, DPI).unwrap()));
    });

    group.bench_function("line_1m_svg", |b| {
        b.iter(|| black_box(line.to_svg(SvgText::Outlines).unwrap()));
    });
    group.bench_function("line_1m_svg_from_scene", |b| {
        b.iter(|| black_box(render_svg(&line_scene, SvgText::Outlines)));
    });
    group.bench_function("line_1m_pdf", |b| {
        b.iter(|| black_box(line.to_pdf().unwrap()));
    });

    let hist = hist_figure(1_000_000);
    group.bench_function("hist_1m_png", |b| {
        b.iter(|| black_box(hist.to_png(i64::from(DPI)).unwrap()));
    });

    // The v0.3 charts: color-mapped marks and a time axis, 6x4 in at 200 dpi.
    let heatmap = heatmap_figure();
    group.bench_function("heatmap_1000x1000_png", |b| {
        b.iter(|| black_box(heatmap.to_png(i64::from(DPI)).unwrap()));
    });
    group.bench_function("heatmap_1000x1000_scene", |b| {
        b.iter(|| black_box(heatmap.to_scene(library).unwrap()));
    });
    group.bench_function("heatmap_1000x1000_svg", |b| {
        b.iter(|| black_box(heatmap.to_svg(SvgText::Outlines).unwrap()));
    });
    group.bench_function("heatmap_1000x1000_pdf", |b| {
        b.iter(|| black_box(heatmap.to_pdf().unwrap()));
    });
    let categories = category_figure();
    group.bench_function("category_200_png", |b| {
        b.iter(|| black_box(categories.to_png(i64::from(DPI)).unwrap()));
    });
    group.bench_function("category_200_scene", |b| {
        b.iter(|| black_box(categories.to_scene(library).unwrap()));
    });
    group.bench_function("category_200_svg", |b| {
        b.iter(|| black_box(categories.to_svg(SvgText::Outlines).unwrap()));
    });
    let hist2d = hist2d_figure(1_000_000);
    group.bench_function("hist2d_1m_png", |b| {
        b.iter(|| black_box(hist2d.to_png(i64::from(DPI)).unwrap()));
    });
    let hexbin = hexbin_figure(1_000_000);
    group.bench_function("hexbin_1m_png", |b| {
        b.iter(|| black_box(hexbin.to_png(i64::from(DPI)).unwrap()));
    });
    let time_line = time_line_figure(1_000_000);
    group.bench_function("time_line_1m_png", |b| {
        b.iter(|| black_box(time_line.to_png(i64::from(DPI)).unwrap()));
    });

    let text = text_figure();
    group.bench_function("text_heavy_png", |b| {
        b.iter(|| black_box(text.to_png(i64::from(DPI)).unwrap()));
    });
    group.finish();
}

/// Best scans the final drawn geometry; stacked areas retain more vertices than a line.
fn legend_benches(c: &mut Criterion) {
    let library = FontLibrary::global();
    let mut group = c.benchmark_group("legend_position");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(5));
    for (kind, n) in [
        ("line_1m", 1_000_000),
        ("area_1m", 1_000_000),
        ("band_1m", 1_000_000),
        ("stacked_area_3x100k", 100_000),
    ] {
        let x = xs(n);
        let y = wave(n);
        for (name, position) in [
            ("top_left", LegendPosition::TopLeft),
            ("best", LegendPosition::Best),
        ] {
            let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
            let mut axes = figure
                .add_axes(ScaleKind::Linear, ScaleKind::Linear)
                .unwrap();
            axes.legend_at(LegendStyle::Header, position, umber_core::SizeSpec::Auto)
                .unwrap();
            match kind {
                "line_1m" => {
                    axes.line(
                        AxisData::Numbers(x.clone()),
                        &y,
                        LineOptions {
                            label: "Signal".into(),
                            ..LineOptions::default()
                        },
                    )
                    .unwrap();
                }
                "area_1m" => {
                    axes.area(
                        AxisData::Numbers(x.clone()),
                        &y,
                        AreaOptions {
                            label: "Signal".into(),
                            ..AreaOptions::default()
                        },
                    )
                    .unwrap();
                }
                "band_1m" => {
                    let upper: Vec<_> = y.iter().map(|v| v + 2.0).collect();
                    axes.band(
                        &x,
                        &y,
                        &upper,
                        BandOptions {
                            label: "Signal".into(),
                            ..BandOptions::default()
                        },
                    )
                    .unwrap();
                }
                "stacked_area_3x100k" => {
                    axes.stacked_area(
                        &x,
                        (0..3)
                            .map(|k| Series {
                                label: format!("Layer {k}"),
                                values: y.iter().map(|v| v + 12.0 + k as f64).collect(),
                                ..Series::default()
                            })
                            .collect(),
                    )
                    .unwrap();
                }
                _ => unreachable!(),
            }
            group.bench_function(format!("{kind}_{name}_scene"), |b| {
                b.iter(|| black_box(figure.to_scene(library).unwrap()));
            });
        }
    }
    group.finish();
}

criterion_group!(render, benches, legend_benches);
criterion_main!(render);
