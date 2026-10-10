//! Character map reading beyond skrifa's own lookup.
//!
//! skrifa maps characters through one best Unicode subtable. A face also consults its other
//! distinct Unicode subtables, and this module finds them and reads them, including format 2
//! subtables, which neither skrifa's lookup nor the shaper reads.

use std::collections::HashSet;

use read_fonts::tables::cmap::{Cmap, Cmap2, CmapSubtable, EncodingRecord, PlatformId};
use read_fonts::types::GlyphId;

/// The most secondary Unicode subtables a face consults, besides the best one.
pub(super) const SECONDARY_SUBTABLES: usize = 16;

/// The distinct Unicode subtables of `cmap` other than the one skrifa's lookup uses, in table
/// order, at most [`SECONDARY_SUBTABLES`] of them.
pub(super) fn secondary_subtables<'a>(cmap: &Cmap<'a>) -> Vec<CmapSubtable<'a>> {
    let best = best_subtable_offset(cmap);
    let mut seen: HashSet<u32> = HashSet::new();
    let mut found = Vec::new();
    for record in cmap.encoding_records() {
        if found.len() == SECONDARY_SUBTABLES {
            break;
        }
        let offset = record.subtable_offset().to_u32();
        if !is_unicode(record) || Some(offset) == best || !seen.insert(offset) {
            continue;
        }
        if let Ok(sub) = record.subtable(cmap.offset_data()) {
            found.push(sub);
        }
    }
    found
}

/// The offset of the subtable skrifa's `Charmap` selects, found the same way: records in
/// reverse order, symbol before full-repertoire before BMP subtables, formats 4, 12 and 13.
fn best_subtable_offset(cmap: &Cmap<'_>) -> Option<u32> {
    let mut best: Option<(u8, u32)> = None;
    for record in cmap.encoding_records().iter().rev() {
        let rank = match (record.platform_id(), record.encoding_id()) {
            (PlatformId::Unicode, 5) => continue,
            (PlatformId::Windows, 0) => 3,
            (PlatformId::Windows, 10) | (PlatformId::Unicode, 4 | 6) => 2,
            (PlatformId::ISO | PlatformId::Unicode, _) | (PlatformId::Windows, 1) => 1,
            _ => continue,
        };
        if best.is_some_and(|(current, _)| rank <= current) {
            continue;
        }
        let supported = matches!(
            record.subtable(cmap.offset_data()),
            Ok(CmapSubtable::Format4(_) | CmapSubtable::Format12(_) | CmapSubtable::Format13(_))
        );
        if supported {
            best = Some((rank, record.subtable_offset().to_u32()));
        }
    }
    best.map(|(_, offset)| offset)
}

/// Whether a `cmap` encoding record holds a Unicode subtable: any Unicode platform encoding,
/// or Windows Unicode BMP (1) and full repertoire (10).
pub(super) fn is_unicode(record: &EncodingRecord) -> bool {
    match record.platform_id() {
        PlatformId::Unicode => true,
        PlatformId::Windows => matches!(record.encoding_id(), 1 | 10),
        _ => false,
    }
}

/// The glyph `sub` maps `cp` to, reading format 2 subtables too; glyph 0 may be returned.
pub(super) fn subtable_glyph(sub: &CmapSubtable<'_>, cp: u32) -> Option<GlyphId> {
    match sub {
        CmapSubtable::Format2(sub) => format2_glyph(sub, cp),
        sub => sub.map_codepoint(cp),
    }
}

/// Looks `cp` up in a format 2 (high-byte mapping) subtable.
///
/// Codes below 0xFF use sub-header 0, as single bytes; others use the sub-header of their high
/// byte. Glyph 0 means no glyph.
fn format2_glyph(sub: &Cmap2<'_>, cp: u32) -> Option<GlyphId> {
    let code = u16::try_from(cp).ok()?;
    let (high, low) = (code >> 8, code & 0xFF);
    let key = if code < 0xFF {
        0
    } else {
        sub.sub_header_keys().get(usize::from(high))?.get() / 8
    };
    let data = sub.offset_data();
    let header = format2_header(key);
    let (first, count) = format2_range(sub, key)?;
    let delta = data.read_at::<i16>(header + 4).ok()?;
    let range_offset = data.read_at::<u16>(header + 6).ok()?;
    if low < first || low >= first.checked_add(count)? {
        return None;
    }
    // The range offset counts bytes from the range offset field itself.
    let at = header + 6 + usize::from(range_offset) + 2 * usize::from(low - first);
    let glyph = data.read_at::<u16>(at).ok()?;
    if glyph == 0 {
        return None;
    }
    // The delta is added modulo 65,536.
    let id = glyph.wrapping_add_signed(delta);
    Some(GlyphId::new(u32::from(id)))
}

/// The byte offset of format 2 sub-header `key` (a sub-header key divided by 8) within the
/// subtable: after the format, length, language and the 256 sub-header keys.
fn format2_header(key: u16) -> usize {
    const HEADERS: usize = 6 + 2 * 256;
    HEADERS + 8 * usize::from(key)
}

/// The first low byte and the number of low bytes that format 2 sub-header `key` (a sub-header
/// key divided by 8) maps; `None` if either cannot be read.
pub(super) fn format2_range(sub: &Cmap2<'_>, key: u16) -> Option<(u16, u16)> {
    let data = sub.offset_data();
    let header = format2_header(key);
    let first = data.read_at::<u16>(header).ok()?;
    let count = data.read_at::<u16>(header + 2).ok()?;
    Some((first, count))
}
