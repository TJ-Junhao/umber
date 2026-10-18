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

fn rectangular_field(width: usize, height: usize, kind: &str) -> CellImage {
    let values: Vec<_> = (0..width * height)
        .map(|i| {
            let col = i % width;
            let row = i / width;
            let x = col as f64 / (width - 1).max(1) as f64;
            let y = row as f64 / (height - 1).max(1) as f64;
            match kind {
                "transparent_blocks" if (col / 100 + row / 100).is_multiple_of(2) => f64::NAN,
                "transparent_blocks" => x + y,
                "noisy_center_band" if (height / 2 - 2..height / 2 + 2).contains(&row) => unit(i),
                "noisy_center_band" => x + y,
                "flat_center_band" if (height / 2 - 2..height / 2 + 2).contains(&row) => 0.5,
                "flat_center_band" => x + y,
                "center_flat_edge_ramp" if (width / 4..3 * width / 4).contains(&col) => 0.5,
                "center_flat_edge_ramp" | "ramp" => x + y,
                "smooth" => (8.0 * x).sin() * (8.0 * y).cos(),
                _ => panic!("unknown rectangular field: {kind}"),
            }
        })
        .collect();
    let x: Vec<_> = (0..=width).map(|i| i as f64).collect();
    let y: Vec<_> = (0..=height).map(|i| i as f64).collect();
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .mesh(
            &x,
            &y,
            &values,
            MeshOptions {
                render: Render::Raster,
                ..MeshOptions::default()
            },
        )
        .unwrap();
    CellImage::from_figure(format!("{kind}_{width}x{height}"), figure)
}

fn category_labels_heatmap() -> CellImage {
    // Match tests/support/corpus/category_labels.rs, including the default render choice.
    let names = |prefix: &str| {
        (0..1000)
            .map(|i| format!("{prefix}{i}"))
            .collect::<Vec<_>>()
    };
    let values: Vec<_> = (0..1_000_000)
        .map(|i| ((i / 1000 + i % 1000) % 100) as f64)
        .collect();
    let mut figure = Figure::new(6.0, 4.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Category, ScaleKind::Category)
        .unwrap()
        .heatmap(&names("x"), &names("y"), &values, HeatmapOptions::default())
        .unwrap();
    CellImage::from_figure("fig-category-labels-heatmap-1000".to_owned(), figure)
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
    for kind in ["row_ramp", "checker", "ij_ramp", "noisy_ramp"] {
        images.push(field(100, kind));
    }
    images.push(field(50, "checker"));
    images.push(category_labels_heatmap());
    for (width, height, kind) in [
        (1000, 1000, "transparent_blocks"),
        (1000, 1000, "noisy_center_band"),
        (1000, 1000, "flat_center_band"),
        (2000, 600, "center_flat_edge_ramp"),
        (8, 3000, "ramp"),
        (3, 4000, "smooth"),
        (1, 20000, "ramp"),
        (3000, 8, "ramp"),
    ] {
        images.push(rectangular_field(width, height, kind));
    }
    // Independent smooth RGB channels cannot be represented by a scalar colormap.
    images.push(CellImage {
        name: "rgb_gradient_1500".to_owned(),
        width: 1500,
        height: 1500,
        pixels: (0..1500 * 1500)
            .map(|i| {
                let x = (i % 1500) as f64 / 1499.0;
                let y = (i / 1500) as f64 / 1499.0;
                Rgba {
                    r: (x * 255.0) as u8,
                    g: (y * 255.0) as u8,
                    b: ((x + y) * 127.5) as u8,
                    a: 255,
                }
            })
            .collect(),
    });
    assert_eq!(images.len(), 35);
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
        let unfiltered = encode(&image, 1, Filter::NoFilter);
        let adaptive = encode(&image, 1, Filter::Adaptive);
        let filter = if adaptive.len() < unfiltered.len() / 2 {
            Filter::Adaptive
        } else {
            Filter::NoFilter
        };
        let mut expected = smaller(unfiltered, adaptive);
        if image.pixels.len() * 4 > 32 * 1024 && expected.len() <= image.pixels.len() * 4 / 3 {
            expected = smaller(expected, encode(&image, 3, filter));
        }
        assert_eq!(after, expected, "{}", image.name);
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
fn embedded_png_preserves_wide_and_tall_images() {
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
        println!(
            "PNG_SIZE\t{}\t{}\t{}",
            image.name,
            before.len(),
            after.len()
        );
        if after.len() * 100 > (before.len() * 110).max((before.len() + 8 * 1024) * 100) {
            failures.push(format!(
                "{}: {} > max(110%, +8 KiB) of {} (+{:.1}%)",
                image.name,
                after.len(),
                before.len(),
                (after.len() as f64 / before.len() as f64 - 1.0) * 100.0
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Alternates the previous encoder and the selected rule to limit timing drift.
/// Run with --release --ignored --nocapture --test-threads=1 on the benchmark machine.
#[test]
#[ignore = "paired compression timings"]
fn measure_selected_embedded_png_corpus() {
    let mut failures = Vec::new();
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
        let (old_ms, new_ms) = (times[0][5], times[1][5]);
        if image.name == "heatmap_1000" && new_ms * 2.0 > old_ms {
            failures.push(format!(
                "{}: {old_ms:.4} / {new_ms:.4} ms is less than 2x faster",
                image.name
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
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
