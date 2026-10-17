//! Embedded PNG compression measurements and exact-byte encoding checks.

use super::{base64, png_bytes};
use crate::scene::Item;
use crate::test_support::image::decode_rgba8;
use crate::{
    Figure, FontLibrary, HeatmapOptions, Hist2dOptions, MeshOptions, Render, Rgba, ScaleKind, Theme,
};
use png::Filter;
use std::hint::black_box;
use std::time::Instant;

struct CellImage {
    name: String,
    width: u32,
    height: u32,
    pixels: Vec<Rgba>,
}

impl CellImage {
    fn from_figure(name: String, figure: Figure) -> Self {
        fn find(items: &[Item]) -> Option<(u32, u32, Vec<Rgba>)> {
            for item in items {
                match item {
                    Item::Image {
                        width,
                        height,
                        pixels,
                        ..
                    } => {
                        return Some((*width, *height, pixels.to_vec()));
                    }
                    Item::Group { items, .. } => {
                        if let Some(image) = find(items) {
                            return Some(image);
                        }
                    }
                    _ => {}
                }
            }
            None
        }
        let scene = figure.to_scene(FontLibrary::global()).unwrap();
        let (width, height, pixels) = find(&scene.items).expect("a raster cell image");
        Self {
            name,
            width,
            height,
            pixels,
        }
    }

    fn rgba(&self) -> Vec<u8> {
        self.pixels
            .iter()
            .flat_map(|c| [c.r, c.g, c.b, c.a])
            .collect()
    }
}

/// The same SplitMix64 values as the render benchmark.
fn unit(i: usize) -> f64 {
    let mut h = (i as u64).wrapping_add(0x9e37_79b9_7f4a_7c15);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^= h >> 31;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

fn heatmap(n: usize, diverging: bool) -> CellImage {
    let names = |prefix: &str| (0..n).map(|i| format!("{prefix}{i}")).collect::<Vec<_>>();
    let values: Vec<_> = (0..n * n)
        .map(|i| {
            if diverging {
                2.0 * unit(i) - 1.0
            } else {
                unit(i)
            }
        })
        .collect();
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Category, ScaleKind::Category)
        .unwrap()
        .heatmap(
            &names("x"),
            &names("y"),
            &values,
            HeatmapOptions {
                center: diverging.then_some(0.0),
                render: Render::Raster,
                ..HeatmapOptions::default()
            },
        )
        .unwrap();
    let kind = if diverging { "diverging" } else { "heatmap" };
    CellImage::from_figure(format!("{kind}_{n}"), figure)
}

fn mesh(n: usize) -> CellImage {
    let edges: Vec<_> = (0..=n).map(|i| i as f64 / n as f64 * 8.0 - 4.0).collect();
    let values: Vec<_> = (0..n * n)
        .map(|i| {
            let x = (edges[i % n] + edges[i % n + 1]) / 2.0;
            let y = (edges[i / n] + edges[i / n + 1]) / 2.0;
            x.sin() * y.cos() + 0.25 * x
        })
        .collect();
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .mesh(
            &edges,
            &edges,
            &values,
            MeshOptions {
                render: Render::Raster,
                ..MeshOptions::default()
            },
        )
        .unwrap();
    CellImage::from_figure(format!("mesh_{n}"), figure)
}

fn field(n: usize, kind: &str) -> CellImage {
    let names: Vec<_> = (0..n).map(|i| i.to_string()).collect();
    let values: Vec<_> = (0..n * n)
        .map(|i| {
            let x = (i % n) as f64 / (n - 1) as f64;
            let y = (i / n) as f64 / (n - 1) as f64;
            let normal = || {
                (-2.0 * (1.0 - unit(2 * i)).ln()).sqrt()
                    * (std::f64::consts::TAU * unit(2 * i + 1)).cos()
            };
            match kind {
                "sin" => (8.0 * x).sin() * (8.0 * y).cos(),
                "row_ramp" => y,
                "ij_ramp" => x + y,
                "noisy_ramp" => x + y + 0.05 * normal(),
                "ramp_noise_001" => x + y + 0.01 * normal(),
                "radial" => (x - 0.5).hypot(y - 0.5),
                "checker" => ((i % n + i / n) % 2) as f64,
                _ => panic!("unknown field: {kind}"),
            }
        })
        .collect();
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Category, ScaleKind::Category)
        .unwrap()
        .heatmap(
            &names,
            &names,
            &values,
            HeatmapOptions {
                render: Render::Raster,
                ..HeatmapOptions::default()
            },
        )
        .unwrap();
    CellImage::from_figure(format!("{kind}_{n}"), figure)
}

fn corpus() -> Vec<CellImage> {
    let mut images = vec![
        heatmap(1000, false),
        heatmap(100, false),
        mesh(100),
        mesh(1000),
    ];
    // Box-Muller normal points, rather than a sum-of-uniforms approximation.
    let (x, y): (Vec<_>, Vec<_>) = (0..1_000_000)
        .map(|i| {
            let r = (-2.0 * (1.0 - unit(2 * i)).ln()).sqrt();
            let angle = std::f64::consts::TAU * unit(2 * i + 1);
            (r * angle.cos(), r * angle.sin())
        })
        .unzip();
    for n in [100, 500] {
        let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
        figure
            .add_axes(ScaleKind::Linear, ScaleKind::Linear)
            .unwrap()
            .hist2d(
                &x,
                &y,
                Hist2dOptions {
                    bins: (n, n),
                    render: Render::Raster,
                    ..Hist2dOptions::default()
                },
            )
            .unwrap();
        let image = CellImage::from_figure(format!("hist2d_{n}"), figure);
        assert!(image.pixels.iter().filter(|c| c.a == 0).count() > image.pixels.len() / 3);
        images.push(image);
    }
    images.push(heatmap(1000, true));
    for n in [100, 1000] {
        // Independent random channels, including alpha and hidden RGB under alpha zero.
        images.push(CellImage {
            name: format!("noise_{n}"),
            width: n,
            height: n,
            pixels: (0..(n * n) as usize)
                .map(|i| Rgba {
                    r: (unit(4 * i) * 256.0) as u8,
                    g: (unit(4 * i + 1) * 256.0) as u8,
                    b: (unit(4 * i + 2) * 256.0) as u8,
                    a: (unit(4 * i + 3) * 256.0) as u8,
                })
                .collect(),
        });
    }
    for (kind, sizes) in [
        ("sin", &[100, 300, 1000][..]),
        ("row_ramp", &[300, 1000][..]),
        ("ij_ramp", &[50][..]),
        ("noisy_ramp", &[20, 50][..]),
        ("radial", &[1000][..]),
        ("ramp_noise_001", &[1000][..]),
        ("checker", &[1000][..]),
    ] {
        for &n in sizes {
            images.push(field(n, kind));
        }
    }
    images
}

fn encode(image: &CellImage, level: u8, filter: Filter) -> Vec<u8> {
    let data = image.rgba();
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_deflate_compression(png::DeflateCompression::Level(level));
    encoder.set_filter(filter);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&data).unwrap();
    writer.finish().unwrap();
    out
}

fn smaller(a: Vec<u8>, b: Vec<u8>) -> Vec<u8> {
    if a.len() <= b.len() { a } else { b }
}

#[test]
fn embedded_png_corpus_preserves_pixels_and_is_deterministic() {
    for image in corpus() {
        let before = encode(&image, 6, Filter::Adaptive);
        let after = png_bytes(image.width, image.height, &image.pixels).unwrap();
        let (level, filter) = match image.name.as_str() {
            "heatmap_1000" | "heatmap_100" | "hist2d_500" | "noise_100" | "noise_1000" => {
                (1, Filter::NoFilter)
            }
            "mesh_100" | "sin_100" | "sin_300" => (3, Filter::Sub),
            "mesh_1000"
            | "hist2d_100"
            | "diverging_1000"
            | "sin_1000"
            | "radial_1000"
            | "ramp_noise_001_1000" => (3, Filter::NoFilter),
            "row_ramp_300" | "row_ramp_1000" | "checker_1000" => (3, Filter::Paeth),
            "ij_ramp_50" | "noisy_ramp_20" | "noisy_ramp_50" => (3, Filter::Adaptive),
            name => panic!("missing expected setting for {name}"),
        };
        assert_eq!(after, encode(&image, level, filter), "{}", image.name);
        let decoded = decode_rgba8(&after).unwrap();
        assert_eq!((decoded.width, decoded.height), (image.width, image.height));
        assert_eq!(decoded.rgba, image.rgba(), "{}", image.name);
        assert_eq!(
            decoded.rgba,
            decode_rgba8(&before).unwrap().rgba,
            "{}",
            image.name
        );
        assert_eq!(
            after,
            png_bytes(image.width, image.height, &image.pixels).unwrap(),
            "{}",
            image.name
        );
    }
}

#[test]
fn embedded_png_sampling_preserves_wide_and_tall_images() {
    for (width, height) in [(6000, 1), (1, 6000), (2000, 3), (3, 2000)] {
        let image = CellImage {
            name: format!("{width}x{height}"),
            width,
            height,
            pixels: (0..(width * height) as usize)
                .map(|i| Rgba {
                    r: (unit(4 * i) * 256.0) as u8,
                    g: (unit(4 * i + 1) * 256.0) as u8,
                    b: (unit(4 * i + 2) * 256.0) as u8,
                    a: (unit(4 * i + 3) * 256.0) as u8,
                })
                .collect(),
        };
        let bytes = png_bytes(width, height, &image.pixels).unwrap();
        assert_eq!(decode_rgba8(&bytes).unwrap().rgba, image.rgba());
        assert_eq!(bytes, png_bytes(width, height, &image.pixels).unwrap());
    }
}

#[test]
fn embedded_png_corpus_size_bound() {
    let mut failures = Vec::new();
    for image in corpus() {
        let before = encode(&image, 6, Filter::Adaptive);
        let after = png_bytes(image.width, image.height, &image.pixels).unwrap();
        if after.len() * 100 > before.len() * 110 {
            failures.push(format!(
                "{}: {} > 110% of {} (+{:.1}%)",
                image.name,
                after.len(),
                before.len(),
                (after.len() as f64 / before.len() as f64 - 1.0) * 100.0
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Run with --release --ignored --nocapture --test-threads=1 on the benchmark machine.
#[test]
#[ignore = "compression timing corpus"]
fn measure_embedded_png_corpus() {
    for image in corpus() {
        let expected = image.rgba();
        for candidate in 0..19 {
            let encode_candidate = || {
                if candidate < 15 {
                    encode(
                        &image,
                        [1, 3, 6][candidate / 5],
                        [
                            Filter::NoFilter,
                            Filter::Sub,
                            Filter::Up,
                            Filter::Paeth,
                            Filter::Adaptive,
                        ][candidate % 5],
                    )
                } else if candidate == 15 {
                    smaller(
                        encode(&image, 1, Filter::NoFilter),
                        encode(&image, 1, Filter::Adaptive),
                    )
                } else if candidate == 16 {
                    smaller(
                        encode(&image, 1, Filter::Sub),
                        encode(&image, 1, Filter::Up),
                    )
                } else if candidate == 17 {
                    smaller(
                        encode(&image, 1, Filter::NoFilter),
                        encode(&image, 3, Filter::Adaptive),
                    )
                } else {
                    png_bytes(image.width, image.height, &image.pixels).unwrap()
                }
            };
            let first = encode_candidate();
            assert_eq!(
                decode_rgba8(&first).unwrap().rgba,
                expected,
                "{} candidate {candidate}",
                image.name
            );
            let mut samples = Vec::new();
            for _ in 0..5 {
                let start = Instant::now();
                let encoded = black_box(encode_candidate());
                samples.push(start.elapsed().as_secs_f64() * 1000.0);
                assert_eq!(encoded, first);
            }
            samples.sort_by(f64::total_cmp);
            println!(
                "PNG_CORPUS\t{}\t{candidate}\t{:.3}\t{}",
                image.name,
                samples[2],
                first.len()
            );
        }
    }
}

/// Alternates the previous encoder and the selected rule to limit timing drift.
#[test]
#[ignore = "paired compression timings"]
fn measure_selected_embedded_png_corpus() {
    for image in corpus() {
        let before = encode(&image, 6, Filter::Adaptive);
        let after = png_bytes(image.width, image.height, &image.pixels).unwrap();
        let mut times = [Vec::new(), Vec::new()];
        for round in 0..11 {
            for turn in 0..2 {
                let variant = (round + turn) % 2;
                let start = Instant::now();
                let bytes = black_box(if variant == 0 {
                    encode(&image, 6, Filter::Adaptive)
                } else {
                    png_bytes(image.width, image.height, &image.pixels).unwrap()
                });
                times[variant].push(start.elapsed().as_secs_f64() * 1000.0);
                assert_eq!(
                    bytes,
                    if variant == 0 { &before } else { &after }.as_slice()
                );
            }
        }
        for samples in &mut times {
            samples.sort_by(f64::total_cmp);
        }
        println!(
            "PNG_SELECTED\t{}\t{:.4}\t{:.4}\t{}\t{}",
            image.name,
            times[0][5],
            times[1][5],
            before.len(),
            after.len()
        );
    }
}

#[test]
fn chunked_base64_matches_previous_encoder_for_all_lengths_to_10000() {
    fn previous(bytes: &[u8]) -> String {
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
        for chunk in bytes.chunks(3) {
            let b = [
                chunk[0],
                chunk.get(1).copied().unwrap_or(0),
                chunk.get(2).copied().unwrap_or(0),
            ];
            let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
            for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
                if i <= chunk.len() {
                    out.push(char::from(ALPHABET[((n >> shift) & 63) as usize]));
                } else {
                    out.push('=');
                }
            }
        }
        out
    }
    let bytes: Vec<_> = (0..10000).map(|i| (unit(i) * 256.0) as u8).collect();
    for len in 0..=bytes.len() {
        assert_eq!(
            base64(&bytes[..len]),
            previous(&bytes[..len]),
            "length {len}"
        );
    }
}
