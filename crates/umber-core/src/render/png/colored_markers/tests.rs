use super::{Cached, MIN_BATCH_ITEMS, draw, draw_run};
use crate::figure::{AxisData, ScatterOptions};
use crate::render::png::{
    Canvas, ClipRect, DeviceScale, FORCE_THREADS, Marker, MarkersView, SpriteKind, Sprites,
    Surface, color, convert_path, draw_markers, normalize_path, prepare_stroke, solid_paint,
};
use crate::{
    Figure, FontLibrary, Item, LineCap, LineJoin, Marker as Shape, Path, Point, Rgba, ScaleKind,
    SizeSpec, StrokeStyle, Theme,
};
use tiny_skia as sk;

fn shape(marker: Shape) -> Path {
    fn find(items: &[Item]) -> Option<Path> {
        items.iter().find_map(|item| match item {
            Item::Markers { shape, .. } => Some(shape.clone()),
            Item::Group { items, .. } => find(items),
            _ => None,
        })
    }
    let mut figure = Figure::new(3.0, 2.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .scatter(
            AxisData::Numbers(vec![0.0, 1.0]),
            &[0.0, 1.0],
            ScatterOptions {
                marker,
                size: SizeSpec::pt(7.0).unwrap(),
                ..ScatterOptions::default()
            },
        )
        .unwrap();
    find(&figure.to_scene(FontLibrary::global()).unwrap().items).unwrap()
}

fn with_surface(dpi: u32, clipped: bool, draw: impl FnOnce(&mut Surface<'_>)) -> Vec<u8> {
    let scale = DeviceScale::new(dpi);
    let mut pixmap = sk::Pixmap::new(240, 180).unwrap();
    pixmap.fill(color(Rgba::opaque(241, 233, 220)));
    let mut canvas = Canvas {
        pixmap,
        scale,
        root: sk::Transform::from_scale(scale.get(), scale.get()),
        mask: None,
    };
    if clipped {
        assert!(canvas.use_clip(Some(ClipRect {
            left: 12.3,
            top: 13.7,
            right: 229.4,
            bottom: 171.2,
        })));
    }
    draw(&mut Surface {
        pixmap: &mut canvas.pixmap,
        scale,
        root: canvas.root,
        clip: canvas.mask.as_ref().map(|(rect, mask)| (mask, *rect)),
    });
    canvas.pixmap.take()
}

fn render(items: &[Item], dpi: u32, clipped: bool, batched: bool) -> Vec<u8> {
    with_surface(dpi, clipped, |surface| {
        if batched {
            assert_eq!(draw(surface, items), items.len());
        } else {
            for item in items {
                if let Item::Markers {
                    shape,
                    fill,
                    stroke,
                    positions,
                } = item
                {
                    draw_markers(
                        surface,
                        &MarkersView {
                            shape,
                            fill: *fill,
                            stroke: stroke.as_ref(),
                            positions,
                        },
                    );
                }
            }
        }
    })
}

fn items(shape: &Path, dpi: u32, cap: LineCap, join: LineJoin) -> Vec<Item> {
    let scale = DeviceScale::new(dpi).get();
    (0..1024)
        .map(|i| {
            let n = i as u8;
            Item::Markers {
                shape: shape.clone(),
                fill: Some(Rgba::opaque(n, n.wrapping_mul(73), n.wrapping_add(121))),
                stroke: Some(StrokeStyle {
                    color: Rgba::opaque(255 - n, n.wrapping_mul(37), n.wrapping_add(11)),
                    width: 0.5,
                    dash: Vec::new(),
                    cap,
                    join,
                }),
                positions: vec![Point {
                    x: ((i * 37 % 240) as f32 + (i % 8) as f32 / 8.0) / scale,
                    y: ((i * 19 % 180) as f32 + (i / 8 % 8) as f32 / 8.0) / scale,
                }],
            }
        })
        .collect()
}

fn assert_exact(items: &[Item], dpi: u32) {
    for clipped in [false, true] {
        let original = render(items, dpi, clipped, false);
        for threads in [1, 8] {
            FORCE_THREADS.with(|value| value.set(Some(threads)));
            let actual = render(items, dpi, clipped, true);
            FORCE_THREADS.with(|value| value.set(None));
            let different = original.iter().zip(&actual).filter(|(a, b)| a != b).count();
            assert_eq!(different, 0, "dpi={dpi}, clip={clipped}, threads={threads}");
        }
    }
}

#[test]
fn shared_sprites_match_original_markers_in_order() {
    for marker in [
        Shape::Circle,
        Shape::Square,
        Shape::Triangle,
        Shape::Diamond,
        Shape::Cross,
    ] {
        let shape = shape(marker);
        for dpi in [72, 100, 144, 145, 200, 300] {
            let items = items(&shape, dpi, LineCap::Butt, LineJoin::Miter);
            assert_exact(&items, dpi);
        }
    }
}

#[test]
fn caps_joins_and_open_paths_match_original_markers() {
    let mut open = Path::new();
    open.move_to(Point { x: -3.5, y: 2.0 });
    open.line_to(Point { x: 0.0, y: -3.5 });
    open.line_to(Point { x: 3.5, y: 2.0 });
    for shape in [open, shape(Shape::Square)] {
        for cap in [LineCap::Butt, LineCap::Round, LineCap::Square] {
            for join in [LineJoin::Miter, LineJoin::Round, LineJoin::Bevel] {
                for dpi in [100, 144, 200] {
                    assert_exact(&items(&shape, dpi, cap, join), dpi);
                }
            }
        }
    }
}

#[test]
fn incompatible_batches_leave_surface_untouched() {
    let base = items(&shape(Shape::Circle), 144, LineCap::Butt, LineJoin::Miter);
    let mut cases = vec![Vec::new(), base[..MIN_BATCH_ITEMS - 1].to_vec()];
    for change in 0..5 {
        let mut items = base.clone();
        for item in &mut items {
            let Item::Markers { fill, stroke, .. } = item else {
                unreachable!();
            };
            match change {
                0 => fill.as_mut().unwrap().a = 254,
                1 => stroke.as_mut().unwrap().color.a = 254,
                2 => stroke.as_mut().unwrap().dash = vec![1.0, 2.0],
                3 => *fill = None,
                _ => *stroke = None,
            }
        }
        cases.push(items);
    }
    let mut short_prefix = base.clone();
    if let Item::Markers { fill, .. } = &mut short_prefix[MIN_BATCH_ITEMS - 1] {
        *fill = None;
    }
    cases.push(short_prefix);
    for dpi in [100, 144, 200] {
        for items in &cases {
            with_surface(dpi, true, |surface| {
                let before = surface.pixmap.data().to_vec();
                assert_eq!(draw(surface, items), 0);
                assert_eq!(surface.pixmap.data(), before);
            });
        }
    }
}

fn prepared(item: &Item, surface: &Surface<'_>) -> Marker {
    let Item::Markers {
        shape,
        fill: Some(fill),
        stroke: Some(style),
        ..
    } = item
    else {
        unreachable!();
    };
    let normalized = normalize_path(shape).unwrap();
    let path = convert_path(&normalized).unwrap();
    let stroke = prepare_stroke(&normalized, &path, style, surface.root).unwrap();
    Marker {
        path,
        fill: Some(solid_paint(*fill)),
        stroke: Some(stroke),
    }
}

#[test]
fn colored_cache_stays_within_its_memory_budget() {
    let items = items(&shape(Shape::Circle), 100, LineCap::Butt, LineJoin::Miter);
    with_surface(100, false, |surface| {
        let marker = prepared(&items[0], surface);
        let mut sprites = Sprites::new(
            &marker,
            surface.scale,
            Cached::new(super::COLORED_CACHE_BYTES),
        )
        .unwrap();
        for i in 0..100_000 {
            let colors = (
                Rgba::opaque(i as u8, (i >> 8) as u8, (i >> 16) as u8),
                Rgba::opaque(251, 249, 242),
            );
            let phase = i % (super::PHASES * super::PHASES);
            if sprites.kind.is_full(phase, colors) {
                break;
            }
            Cached::prepare(&mut sprites, &marker, surface.scale, phase, colors).unwrap();
        }
        assert_eq!(sprites.phases.len(), sprites.kind.limit);
        Cached::materialize(&mut sprites, &marker, surface.scale);
        let pixels_and_spans: usize = sprites
            .phases
            .iter()
            .map(|sprite| {
                let sprite = sprite.as_ref().unwrap();
                sprite.pixmap.data().len()
                    + sprite.spans.capacity() * std::mem::size_of::<std::ops::Range<usize>>()
            })
            .sum();
        let buckets = (sprites.kind.indices.capacity() + 1).next_power_of_two();
        let bytes = pixels_and_spans
            + sprites.phases.capacity() * std::mem::size_of::<Option<super::Sprite>>()
            + sprites.kind.pending.capacity() * std::mem::size_of::<super::CacheKey>()
            + buckets * (std::mem::size_of::<(super::CacheKey, usize)>() + 1)
            + 16;
        assert!(
            bytes <= super::COLORED_CACHE_BYTES,
            "cache uses {bytes} bytes"
        );
    });
}

#[test]
fn incompatible_suffix_is_left_for_the_original_renderer() {
    for dpi in [100, 144, 200] {
        let mut items = items(&shape(Shape::Circle), dpi, LineCap::Butt, LineJoin::Miter);
        if let Item::Markers { shape: path, .. } = &mut items[MIN_BATCH_ITEMS] {
            *path = shape(Shape::Square);
        }
        let actual = with_surface(dpi, true, |surface| {
            assert_eq!(draw(surface, &items), MIN_BATCH_ITEMS);
        });
        assert_eq!(actual, render(&items[..MIN_BATCH_ITEMS], dpi, true, false));
    }
}

#[test]
fn colored_cache_reuses_colors_and_phases_and_evicts_after_flushing() {
    let items = items(&shape(Shape::Circle), 144, LineCap::Round, LineJoin::Round);
    with_surface(144, false, |surface| {
        let marker = prepared(&items[0], surface);
        let mut sprites = Sprites::new(&marker, surface.scale, Cached::new(4096)).unwrap();
        sprites.kind.byte_limit = 2 * Cached::sprite_bytes(sprites.size);
        let colors = (Rgba::opaque(0, 73, 121), Rgba::opaque(255, 37, 11));
        let a = Cached::prepare(&mut sprites, &marker, surface.scale, 0, colors).unwrap();
        let b = Cached::prepare(&mut sprites, &marker, surface.scale, 0, colors).unwrap();
        assert_eq!(a, b);
        let c = Cached::prepare(&mut sprites, &marker, surface.scale, 1, colors).unwrap();
        assert_ne!(a, c);
        assert_eq!(sprites.kind.indices.len(), 2);
        assert_eq!(sprites.kind.limit, 2);
        assert!(!sprites.kind.is_full(0, colors));
        assert!(sprites.kind.is_full(2, colors));
        assert!(sprites.kind.is_full(0, (colors.1, colors.0)));
    });
    for dpi in [100, 144] {
        for clipped in [false, true] {
            let original = render(&items, dpi, clipped, false);
            for threads in [1, 8] {
                FORCE_THREADS.with(|value| value.set(Some(threads)));
                let actual = with_surface(dpi, clipped, |surface| {
                    let marker = prepared(&items[0], surface);
                    assert_eq!(
                        draw_run(surface, &items, marker, Cached::new(4096)),
                        items.len()
                    );
                });
                FORCE_THREADS.with(|value| value.set(None));
                assert_eq!(
                    original, actual,
                    "dpi={dpi}, clip={clipped}, threads={threads}"
                );
            }
        }
    }
}
