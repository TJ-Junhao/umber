//! Shared sprites for consecutive opaque marker batches, without reordering scene items.

use std::collections::HashMap;
use std::mem::size_of;
use std::ops::Range;

use tiny_skia as sk;

use super::{
    DeviceScale, Marker, PHASES, Sprite, SpriteKind, Sprites, Surface, convert_path, div255,
    normalize_path, place_markers, prepare_stroke, solid_paint, sprites_disabled,
};
use crate::{Item, Rgba};

#[cfg(test)]
mod tests;

/// Amortizes shared-sprite setup over at least one complete 8 x 8 sub-pixel phase set.
const MIN_BATCH_ITEMS: usize = PHASES * PHASES;

/// 16 MiB holds thousands of small, already-colored sprites without scaling with point count.
/// The budget includes pixels, row spans and conservative vector/hash-table storage estimates.
const COLORED_CACHE_BYTES: usize = 16 * 1024 * 1024;

/// The prefix of marker items with the same geometry and opaque fills and outlines.
fn matching(items: &[Item]) -> usize {
    let Some(Item::Markers {
        shape,
        fill: Some(fill),
        stroke: Some(stroke),
        ..
    }) = items.first()
    else {
        return 0;
    };
    if fill.a != 255 || stroke.color.a != 255 || !stroke.dash.is_empty() {
        return 0;
    }
    items
        .iter()
        .take_while(|item| {
            #[cfg(test)]
            tests::MATCHING_ITEMS.with(|count| count.set(count.get() + 1));
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
    if matching(&items[..items.len().min(MIN_BATCH_ITEMS)]) < MIN_BATCH_ITEMS {
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
    let marker = Marker {
        path,
        fill: Some(solid_paint(Rgba::opaque(255, 255, 255))),
        stroke: Some(stroke),
    };
    // Hairlines can visit a pixel more than once: retain the original colored rendering,
    // rather than trying to recover those visits from their combined coverage.
    if style.width * surface.scale.get() <= 1.0 {
        draw_run(surface, items, marker, Cached::new(COLORED_CACHE_BYTES))
    } else {
        draw_run(surface, items, marker, Coverage)
    }
}

fn draw_run<K: SpriteKind<Colors = (Rgba, Rgba)>>(
    surface: &mut Surface<'_>,
    items: &[Item],
    mut marker: Marker,
    kind: K,
) -> usize {
    let Some(sprites) = Sprites::new(&marker, surface.scale, kind) else {
        return 0;
    };
    // Scan the full run only when we can consume it, not once per oversized marker.
    let items = &items[..matching(items)];
    let mut sprites = Some(sprites);
    let mut queue = Vec::new();
    for item in items {
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
        place_markers(
            surface,
            &marker,
            positions,
            &mut sprites,
            &mut queue,
            (*fill, style.color),
        );
    }
    if let Some(mut sprites) = sprites {
        sprites.flush(surface.pixmap, &mut queue, &marker, surface.scale);
    }
    items.len()
}

/// Geometry, size and scale belong to the set; only colors and phase vary within it.
type CacheKey = (Rgba, Rgba, usize);

struct Cached {
    indices: HashMap<CacheKey, usize>,
    pending: Vec<CacheKey>,
    byte_limit: usize,
    limit: usize,
}

impl Cached {
    fn new(byte_limit: usize) -> Self {
        Self {
            indices: HashMap::new(),
            pending: Vec::new(),
            byte_limit,
            limit: 0,
        }
    }

    fn sprite_bytes((width, height): (u32, u32)) -> usize {
        let pixels = width as usize * height as usize * 4;
        let spans = height as usize * size_of::<Range<usize>>();
        // Vectors can reserve twice their length; hash tables have spare buckets and control
        // bytes. Four buckets per entry also cover growth rounding at small capacities.
        pixels
            + spans
            + 2 * (size_of::<Option<Sprite>>() + size_of::<CacheKey>())
            + 4 * (size_of::<(CacheKey, usize)>() + 1)
    }
}

impl SpriteKind for Cached {
    type Colors = (Rgba, Rgba);

    fn prepare(
        sprites: &mut Sprites<Self>,
        _marker: &Marker,
        _scale: DeviceScale,
        phase: usize,
        (fill, stroke): Self::Colors,
    ) -> Option<usize> {
        let key = (fill, stroke, phase);
        if let Some(&index) = sprites.kind.indices.get(&key) {
            return Some(index);
        }
        if sprites.kind.limit == 0 {
            sprites.kind.limit =
                (sprites.kind.byte_limit / Self::sprite_bytes(sprites.size)).max(1);
        }
        let index = sprites.phases.len();
        sprites.phases.push(None);
        sprites.kind.pending.push(key);
        sprites.kind.indices.insert(key, index);
        Some(index)
    }

    fn materialize(sprites: &mut Sprites<Self>, marker: &Marker, scale: DeviceScale) {
        let count = sprites.kind.pending.len();
        if count == 0 {
            return;
        }
        let transforms: Vec<_> = (0..PHASES * PHASES)
            .map(|phase| sprites.transform(scale, phase))
            .collect();
        let size = sprites.size;
        let render = |slots: &mut [Option<Sprite>], keys: &[CacheKey]| {
            let mut marker = marker.clone();
            for (slot, &(fill, outline, phase)) in slots.iter_mut().zip(keys) {
                marker.fill = Some(solid_paint(fill));
                if let Some(stroke) = &mut marker.stroke {
                    stroke.paint = solid_paint(outline);
                }
                if let Some(mut pixmap) = sk::Pixmap::new(size.0, size.1) {
                    marker.draw(&mut pixmap, transforms[phase], None);
                    *slot = Some(Sprite::new(pixmap));
                }
            }
        };
        let start = sprites.phases.len() - count;
        let slots = &mut sprites.phases[start..];
        let keys = &sprites.kind.pending;
        let threads = preparing_threads(count);
        if threads == 1 {
            render(slots, keys);
        } else {
            let chunk = count.div_ceil(threads);
            std::thread::scope(|scope| {
                for (slots, keys) in slots.chunks_mut(chunk).zip(keys.chunks(chunk)) {
                    let render = &render;
                    scope.spawn(move || render(slots, keys));
                }
            });
        }
        sprites.kind.pending.clear();
    }

    fn blend_row(dst: &mut [u8], src: &[u8], _: Self::Colors) {
        super::blend_row(dst, src);
    }

    fn is_full(&self, phase: usize, (fill, stroke): Self::Colors) -> bool {
        !self.indices.is_empty()
            && self.indices.len() >= self.limit
            && !self.indices.contains_key(&(fill, stroke, phase))
    }

    fn clear(&mut self) {
        // Evict a whole generation after flushing its placements, not on each cache miss:
        // otherwise a full cache would force thousands of tiny compositing batches.
        self.indices.clear();
    }
}

/// Rasterizing a sprite costs much more than compositing it; amortize each worker over at
/// least 128 new sprites, with at most 16 workers to bound thread setup and scratch memory.
fn preparing_threads(pending: usize) -> usize {
    #[cfg(test)]
    if let Some(threads) = super::FORCE_THREADS.with(std::cell::Cell::get) {
        return threads;
    }
    static AVAILABLE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    let available = *AVAILABLE.get_or_init(|| {
        std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
    });
    (pending / 128).clamp(1, available.clamp(1, 16))
}

struct Coverage;

impl SpriteKind for Coverage {
    type Colors = (Rgba, Rgba);

    fn prepare(
        sprites: &mut Sprites<Self>,
        marker: &Marker,
        scale: DeviceScale,
        phase: usize,
        _: Self::Colors,
    ) -> Option<usize> {
        sprites.phases.resize_with(PHASES * PHASES, || None);
        if sprites.phases[phase].is_none() {
            sprites.phases[phase] = Some(prepare_coverage(marker, sprites, scale, phase)?);
        }
        Some(phase)
    }

    fn blend_row(dst: &mut [u8], src: &[u8], (fill, stroke): Self::Colors) {
        blend_row(dst, src, fill, stroke);
    }
}

/// Stores fill coverage, stroke coverage and their combined alpha, independent of color.
fn prepare_coverage(
    marker: &Marker,
    sprites: &Sprites<Coverage>,
    scale: DeviceScale,
    phase: usize,
) -> Option<Sprite> {
    let mut fill = sk::Pixmap::new(sprites.size.0, sprites.size.1)?;
    let mut outline = sk::Pixmap::new(sprites.size.0, sprites.size.1)?;
    let at = sprites.transform(scale, phase);
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
fn blend_row(dst: &mut [u8], coverage: &[u8], fill: Rgba, stroke: Rgba) {
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
