//! Shared coverage for consecutive opaque marker batches, without reordering scene items.

use super::*;

#[cfg(test)]
mod tests;

/// The prefix of marker items with the same geometry and opaque fills and outlines.
fn matching(items: &[Item], scale: DeviceScale) -> usize {
    let Some(Item::Markers {
        shape,
        fill: Some(fill),
        stroke: Some(stroke),
        ..
    }) = items.first()
    else {
        return 0;
    };
    // Hairline strokes can visit a pixel more than once; their final alpha is not a coverage.
    if fill.a != 255
        || stroke.color.a != 255
        || stroke.width * scale.get() <= 1.0
        || !stroke.dash.is_empty()
    {
        return 0;
    }
    items
        .iter()
        .take_while(|item| {
            matches!(item, Item::Markers {
                shape: other,
                fill: Some(fill),
                stroke: Some(style),
                ..
            } if other == shape && fill.a == 255 && style.color.a == 255
                && style.width == stroke.width && style.cap == stroke.cap
                && style.join == stroke.join && style.dash.is_empty())
        })
        .count()
}

/// Draws a compatible prefix, or leaves it to the ordinary marker path.
/// SVG, PDF and the scene keep their original batches.
pub(super) fn draw(surface: &mut Surface<'_>, items: &[Item]) -> usize {
    if sprites_disabled() {
        return 0;
    }
    if matching(&items[..items.len().min(64)], surface.scale) < 64 {
        return 0;
    }
    let Item::Markers {
        shape,
        stroke: Some(style),
        ..
    } = &items[0]
    else {
        return 0;
    };
    let Some(normalized) = normalize_path(shape) else {
        return 0;
    };
    let Some(path) = convert_path(&normalized) else {
        return 0;
    };
    let Some(stroke) = prepare_stroke(&normalized, &path, style, surface.root) else {
        return 0;
    };
    let mut marker = Marker {
        path,
        fill: Some(solid_paint(Rgba::opaque(255, 255, 255))),
        stroke: Some(stroke),
    };
    let Some(mut sprites) = Sprites::new(&marker, surface.scale) else {
        return 0;
    };
    let count = matching(items, surface.scale);
    let (scale, root, clip) = (surface.scale, surface.root, surface.clip);
    let pixmap = &mut *surface.pixmap;
    let (width, height) = (pixmap.width() as i32, pixmap.height() as i32);
    let mask = clip.map(|(mask, _)| mask);
    let interior = clip.map(|(_, rect)| rect.interior(pixmap.width(), pixmap.height()));
    let mut queue = Vec::new();
    for item in &items[..count] {
        let Item::Markers {
            fill: Some(fill),
            stroke: Some(style),
            positions,
            ..
        } = item
        else {
            continue;
        };
        marker.fill = Some(solid_paint(*fill));
        if let Some(stroke) = &mut marker.stroke {
            stroke.paint = solid_paint(style.color);
        }
        for p in positions.iter().filter(|p| valid_point(**p)) {
            let (x, px) = split_phase(scale.to_device(p.x));
            let (y, py) = split_phase(scale.to_device(p.y));
            let (left, top) = (x - sprites.origin.0, y - sprites.origin.1);
            let (w, h) = (sprites.size.0 as i32, sprites.size.1 as i32);
            if left >= width || top >= height || left + w <= 0 || top + h <= 0 {
                continue;
            }
            let phase = py * PHASES + px;
            let Some(mut placement) = Placement::unmasked(interior, phase, (left, top), (w, h))
            else {
                sprites.flush(pixmap, &mut queue);
                marker.draw(pixmap, root.pre_translate(p.x, p.y), mask);
                continue;
            };
            if sprites.phases[phase].is_none() {
                let Some(sprite) = prepare_coverage(&marker, &sprites, scale, phase) else {
                    continue;
                };
                sprites.phases[phase] = Some(sprite);
            }
            placement.colors = Some((*fill, style.color));
            queue.push(placement);
            if queue.len() == QUEUE_LIMIT {
                sprites.flush(pixmap, &mut queue);
            }
        }
    }
    sprites.flush(pixmap, &mut queue);
    count
}

/// Stores fill coverage, stroke coverage and their combined alpha, independent of color.
fn prepare_coverage(
    marker: &Marker,
    sprites: &Sprites,
    scale: DeviceScale,
    phase: usize,
) -> Option<Sprite> {
    let mut fill = sk::Pixmap::new(sprites.size.0, sprites.size.1)?;
    let mut outline = sk::Pixmap::new(sprites.size.0, sprites.size.1)?;
    let step = 1.0 / PHASES as f32;
    let at = sk::Transform::from_scale(scale.get(), scale.get()).post_translate(
        sprites.origin.0 as f32 + (phase % PHASES) as f32 * step,
        sprites.origin.1 as f32 + (phase / PHASES) as f32 * step,
    );
    let white = solid_paint(Rgba::opaque(255, 255, 255));
    fill.fill_path(&marker.path, &white, sk::FillRule::Winding, at, None);
    if let Some(stroke) = &marker.stroke {
        outline.stroke_path(&stroke.path, &white, &stroke.stroke, at, None);
    }
    for (pixel, stroke) in fill
        .data_mut()
        .chunks_exact_mut(4)
        .zip(outline.data().chunks_exact(4))
    {
        let (f, s) = (u32::from(pixel[3]), u32::from(stroke[3]));
        pixel[0] = f as u8;
        pixel[1] = s as u8;
        pixel[2] = 0;
        pixel[3] = ((f * (255 - s) + 255 * s + 255) >> 8) as u8;
    }
    Some(Sprite::new(fill))
}

/// tiny-skia's lowp opaque Source pipeline: fill on transparent, then stroke with coverage.
/// Its coverage division is `(v + 255) >> 8`, not the sprite compositor's rounded /255.
#[inline(never)]
pub(super) fn blend_row(dst: &mut [u8], coverage: &[u8], fill: Rgba, stroke: Rgba) {
    let fill = [fill.r, fill.g, fill.b, 255].map(u32::from);
    let stroke = [stroke.r, stroke.g, stroke.b, 255].map(u32::from);
    for (pixel, mask) in dst.chunks_exact_mut(4).zip(coverage.chunks_exact(4)) {
        let (f, s, alpha) = (u32::from(mask[0]), u32::from(mask[1]), u32::from(mask[3]));
        for ((channel, fill), stroke) in pixel.iter_mut().zip(fill).zip(stroke) {
            let under = (fill * f + 255) >> 8;
            let source = (under * (255 - s) + stroke * s + 255) >> 8;
            *channel = (source + div255(u32::from(*channel) * (255 - alpha))).min(255) as u8;
        }
    }
}
