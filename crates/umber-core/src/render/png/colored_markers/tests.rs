use super::*;
use crate::figure::{AxisData, ScatterOptions};
use crate::{Figure, FontLibrary, Marker as Shape, ScaleKind, SizeSpec, Theme};

fn shape(marker: Shape) -> crate::Path {
    fn find(items: &[Item]) -> Option<crate::Path> {
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

fn render(items: &[Item], dpi: u32, clipped: bool, batched: bool) -> Vec<u8> {
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
    let mut surface = Surface {
        pixmap: &mut canvas.pixmap,
        scale,
        root: canvas.root,
        clip: canvas.mask.as_ref().map(|(rect, mask)| (mask, *rect)),
    };
    if batched {
        assert_eq!(draw(&mut surface, items), items.len());
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
                    &mut surface,
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
    canvas.pixmap.take()
}

#[test]
fn shared_coverage_matches_original_markers_in_order() {
    for marker in [
        Shape::Circle,
        Shape::Square,
        Shape::Triangle,
        Shape::Diamond,
        Shape::Cross,
    ] {
        let shape = shape(marker);
        for dpi in [144, 200, 300] {
            let scale = DeviceScale::new(dpi).get();
            let items: Vec<_> = (0..1024)
                .map(|i| {
                    let n = i as u8;
                    Item::Markers {
                        shape: shape.clone(),
                        fill: Some(Rgba::opaque(n, n.wrapping_mul(73), n.wrapping_add(121))),
                        stroke: Some(StrokeStyle {
                            color: Rgba::opaque(255 - n, n.wrapping_mul(37), n.wrapping_add(11)),
                            width: if dpi == 144 { 0.8 } else { 0.5 },
                            dash: Vec::new(),
                            cap: LineCap::Butt,
                            join: LineJoin::Miter,
                        }),
                        positions: vec![Point {
                            x: ((i * 37 % 240) as f32 + (i % 8) as f32 / 8.0) / scale,
                            y: ((i * 19 % 180) as f32 + (i / 8 % 8) as f32 / 8.0) / scale,
                        }],
                    }
                })
                .collect();
            for clipped in [false, true] {
                let original = render(&items, dpi, clipped, false);
                for threads in [1, 8] {
                    FORCE_THREADS.with(|value| value.set(Some(threads)));
                    let actual = render(&items, dpi, clipped, true);
                    FORCE_THREADS.with(|value| value.set(None));
                    let different = original.iter().zip(&actual).filter(|(a, b)| a != b).count();
                    assert_eq!(
                        different, 0,
                        "{marker:?}, dpi={dpi}, clip={clipped}, threads={threads}"
                    );
                }
            }
        }
    }
}
