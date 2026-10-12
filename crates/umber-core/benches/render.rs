//! Render benchmarks: the spec's two timing targets plus SVG, histogram and text-heavy cases.
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
use umber_core::{Figure, FontLibrary, LegendStyle, ScaleKind, Theme};

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

/// Long title and subtitle, axis labels, a legend with four series and many tick labels.
fn text_figure() -> Figure {
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    let axes = figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap();
    axes.title("Quarterly station output across the northern and southern grids")
        .subtitle("Measured every fifteen minutes between January and December, after calibration")
        .xlabel("Elapsed time since the first measurement (days)")
        .ylabel("Output per station (megawatt hours)")
        .legend(LegendStyle::Header);
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

    let hist = hist_figure(1_000_000);
    group.bench_function("hist_1m_png", |b| {
        b.iter(|| black_box(hist.to_png(i64::from(DPI)).unwrap()));
    });

    let text = text_figure();
    group.bench_function("text_heavy_png", |b| {
        b.iter(|| black_box(text.to_png(i64::from(DPI)).unwrap()));
    });
    group.finish();
}

criterion_group!(render, benches);
criterion_main!(render);
