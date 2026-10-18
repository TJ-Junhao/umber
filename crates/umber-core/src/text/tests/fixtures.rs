//! Fonts and helpers shared by the text suites.
//!
//! Test fonts are built from `fallback-only.ttf`, a font with one outlined glyph, by replacing
//! its tables: a new name, weight and style, a chosen `cmap`, or a CFF outline table.

use crate::fonts::{BundledFont, Weight, font_bytes};
use crate::test_support::{bundled_face, bundled_fonts};
use crate::text::font::Face;
use crate::text::{FontLibrary, TextLayout, TextStyle};

/// A regular style in the bundled font `font`, as a face of [`bundled_fonts`].
pub(super) fn style(font: BundledFont, size: f32) -> TextStyle {
    TextStyle {
        face: bundled_face(font),
        weight: Weight::Regular,
        size,
        tabular_figures: false,
    }
}

/// Layout through a library with every bundled font as the fonts of the scene
/// ([`bundled_fonts`]), so that a style from [`style`] names its bundled font.
pub(super) trait LayoutBundled {
    /// Shapes `text` with `style` and measures it.
    fn layout(&self, text: &str, style: &TextStyle) -> TextLayout;
}

impl LayoutBundled for FontLibrary {
    fn layout(&self, text: &str, style: &TextStyle) -> TextLayout {
        bundled_fonts(self).layout(text, style)
    }
}

pub(super) fn omega_lib() -> FontLibrary {
    FontLibrary::with_fallbacks(vec![
        include_bytes!("../../../tests/fixtures/fallback-only.ttf").to_vec(),
    ])
}

pub(super) fn be16(data: &[u8], at: usize) -> usize {
    usize::from(u16::from_be_bytes([data[at], data[at + 1]]))
}

pub(super) fn be32(data: &[u8], at: usize) -> usize {
    u32::from_be_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]) as usize
}

/// A `cmap` table whose `records` encoding records (Windows, Unicode full repertoire) all
/// point at one format 12 subtable that maps every code point in `codepoints` to glyph 1.
pub(super) fn format12_cmap(codepoints: &[u32], records: u16) -> Vec<u8> {
    let mut points = codepoints.to_vec();
    points.sort_unstable();
    points.dedup();
    // One group per code point: a format 12 group maps consecutive code points to
    // consecutive glyphs, and every code point here must map to glyph 1.
    let groups: Vec<(u32, u32)> = points.iter().map(|&cp| (cp, cp)).collect();
    let mut cmap = Vec::new();
    cmap.extend(0u16.to_be_bytes()); // version
    cmap.extend(records.to_be_bytes());
    let offset = 4 + 8 * u32::from(records);
    for _ in 0..records {
        cmap.extend(3u16.to_be_bytes()); // Windows
        cmap.extend(10u16.to_be_bytes()); // Unicode full repertoire
        cmap.extend(offset.to_be_bytes());
    }
    cmap.extend(12u16.to_be_bytes()); // format 12
    cmap.extend(0u16.to_be_bytes());
    cmap.extend((16 + 12 * groups.len() as u32).to_be_bytes());
    cmap.extend(0u32.to_be_bytes()); // language
    cmap.extend((groups.len() as u32).to_be_bytes());
    for (lo, hi) in groups {
        cmap.extend(lo.to_be_bytes());
        cmap.extend(hi.to_be_bytes());
        cmap.extend(1u32.to_be_bytes()); // glyph 1, the outlined glyph
    }
    cmap
}

/// `fallback-only.ttf` rebuilt with a new name, weight and style, and its one outlined glyph
/// mapped to every code point in `codepoints`.
pub(super) fn fixture_font(family: &str, weight: u16, italic: bool, codepoints: &[u32]) -> Vec<u8> {
    font_with_cmap(family, weight, italic, format12_cmap(codepoints, 1))
}

/// The tables of a font, in directory order.
pub(super) fn tables_of(font: &[u8]) -> Vec<([u8; 4], Vec<u8>)> {
    (0..be16(font, 4))
        .map(|i| {
            let record = 12 + 16 * i;
            let tag: [u8; 4] = font[record..record + 4].try_into().unwrap();
            let (offset, length) = (be32(font, record + 8), be32(font, record + 12));
            (tag, font[offset..offset + length].to_vec())
        })
        .collect()
}

/// A TrueType font file holding `tables`.
pub(super) fn assemble(mut tables: Vec<([u8; 4], Vec<u8>)>) -> Vec<u8> {
    tables.sort_by_key(|(tag, _)| *tag);
    let count = tables.len();
    let selector = usize::BITS - 1 - count.leading_zeros();
    let search_range = (1usize << selector) * 16;
    let mut font = Vec::new();
    font.extend(0x0001_0000u32.to_be_bytes());
    for field in [
        count,
        search_range,
        selector as usize,
        count * 16 - search_range,
    ] {
        font.extend((field as u16).to_be_bytes());
    }
    let mut offset = 12 + 16 * count;
    let mut body = Vec::new();
    for (tag, data) in &tables {
        let mut padded = data.clone();
        padded.resize(data.len().div_ceil(4) * 4, 0);
        let checksum = padded
            .chunks(4)
            .map(|w| u32::from_be_bytes(w.try_into().unwrap()))
            .fold(0u32, u32::wrapping_add);
        font.extend(tag);
        font.extend(checksum.to_be_bytes());
        font.extend((offset as u32).to_be_bytes());
        font.extend((data.len() as u32).to_be_bytes());
        offset += padded.len();
        body.extend(padded);
    }
    font.extend(body);
    font
}

/// `font` with the table `tag` added or replaced.
pub(super) fn with_table(font: &[u8], tag: [u8; 4], data: Vec<u8>) -> Vec<u8> {
    let mut tables = tables_of(font);
    tables.retain(|(t, _)| *t != tag);
    tables.push((tag, data));
    assemble(tables)
}

/// `fallback-only.ttf` rebuilt with a new name, weight and style, and the given `cmap`.
pub(super) fn font_with_cmap(family: &str, weight: u16, italic: bool, cmap: Vec<u8>) -> Vec<u8> {
    let base: &[u8] = include_bytes!("../../../tests/fixtures/fallback-only.ttf");
    assert_eq!(be32(base, 0), 0x0001_0000);
    let mut tables = tables_of(base);

    let post_script = format!(
        "{}-{weight}{}",
        family.replace(' ', ""),
        if italic { "Italic" } else { "" }
    );
    let names = [
        (1u16, family.to_string()),
        (2, "Regular".into()),
        (4, family.into()),
        (6, post_script),
    ];
    let mut strings = Vec::new();
    let mut name = Vec::new();
    name.extend(0u16.to_be_bytes());
    name.extend((names.len() as u16).to_be_bytes());
    name.extend((6 + 12 * names.len() as u16).to_be_bytes());
    for (id, text) in &names {
        let encoded: Vec<u8> = text.encode_utf16().flat_map(u16::to_be_bytes).collect();
        for field in [
            3u16,
            1,
            0x409,
            *id,
            encoded.len() as u16,
            strings.len() as u16,
        ] {
            name.extend(field.to_be_bytes());
        }
        strings.extend(encoded);
    }
    name.extend(strings);

    for (tag, data) in &mut tables {
        match &*tag {
            b"cmap" => *data = cmap.clone(),
            b"name" => *data = name.clone(),
            b"OS/2" => {
                data[4..6].copy_from_slice(&weight.to_be_bytes());
                let mut selection = u16::from_be_bytes([data[62], data[63]]);
                selection = if italic {
                    (selection | 1) & !(1 << 6)
                } else {
                    selection & !1
                };
                data[62..64].copy_from_slice(&selection.to_be_bytes());
            }
            b"post" => {
                let angle: i32 = if italic { -12 << 16 } else { 0 };
                data[4..8].copy_from_slice(&angle.to_be_bytes());
            }
            _ => {}
        }
    }
    assemble(tables)
}

/// A format 12 subtable with the given `(first, last, glyph)` groups.
pub(super) fn format12(groups: &[(u32, u32, u32)]) -> Vec<u8> {
    let mut sub = Vec::new();
    sub.extend(12u16.to_be_bytes());
    sub.extend(0u16.to_be_bytes());
    sub.extend((16 + 12 * groups.len() as u32).to_be_bytes());
    sub.extend(0u32.to_be_bytes());
    sub.extend((groups.len() as u32).to_be_bytes());
    for &(first, last, glyph) in groups {
        for field in [first, last, glyph] {
            sub.extend(field.to_be_bytes());
        }
    }
    sub
}

/// A `cmap` table with the given `(platform, encoding, subtable)` records; each subtable
/// is stored once, however many records point at it.
pub(super) fn cmap_of(subtables: &[Vec<u8>], records: &[(u16, u16, usize)]) -> Vec<u8> {
    let mut offsets = Vec::new();
    let mut offset = 4 + 8 * records.len();
    for sub in subtables {
        offsets.push(offset as u32);
        offset += sub.len();
    }
    let mut cmap = Vec::new();
    cmap.extend(0u16.to_be_bytes());
    cmap.extend((records.len() as u16).to_be_bytes());
    for &(platform, encoding, sub) in records {
        cmap.extend(platform.to_be_bytes());
        cmap.extend(encoding.to_be_bytes());
        cmap.extend(offsets[sub].to_be_bytes());
    }
    for sub in subtables {
        cmap.extend(sub);
    }
    cmap
}

/// Empties the `glyf` and `loca` tables, so the font still claims its code points but
/// has no outlines.
pub(super) fn without_outlines(mut bytes: Vec<u8>) -> Vec<u8> {
    for i in 0..be16(&bytes, 4) {
        let at = 12 + 16 * i;
        if &bytes[at..at + 4] == b"glyf" || &bytes[at..at + 4] == b"loca" {
            bytes[at + 12..at + 16].copy_from_slice(&0u32.to_be_bytes());
        }
    }
    bytes
}

/// A CFF font with three glyphs: an empty `.notdef`, an empty glyph for U+0300 and a
/// square for U+F0010 and U+0301; no GDEF, GSUB or GPOS.
pub(super) fn cff_mark_font() -> Vec<u8> {
    fn index(items: &[Vec<u8>]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend((items.len() as u16).to_be_bytes());
        if items.is_empty() {
            return out;
        }
        out.push(4);
        let mut offset = 1u32;
        out.extend(offset.to_be_bytes());
        for item in items {
            offset += item.len() as u32;
            out.extend(offset.to_be_bytes());
        }
        for item in items {
            out.extend(item);
        }
        out
    }
    fn dict_int(value: usize) -> Vec<u8> {
        let mut out = vec![29];
        out.extend((value as i32).to_be_bytes());
        out
    }
    fn number(value: i16) -> Vec<u8> {
        let mut out = vec![28];
        out.extend(value.to_be_bytes());
        out
    }
    const ENDCHAR: u8 = 14;
    let mut square = Vec::new();
    for (args, op) in [
        (&[100i16, 100][..], 21u8),
        (&[800], 6),
        (&[1200], 7),
        (&[-800], 6),
    ] {
        for &arg in args {
            square.extend(number(arg));
        }
        square.push(op);
    }
    square.push(ENDCHAR);
    let charstrings = index(&[vec![ENDCHAR], vec![ENDCHAR], square]);
    // defaultWidthX 0.
    let private = vec![139, 20];
    let header = vec![1, 0, 4, 4];
    let name = index(&[b"Test".to_vec()]);
    let top = |charstrings_at: usize, private_at: usize| {
        let mut dict = dict_int(charstrings_at);
        dict.push(17);
        dict.extend(dict_int(private.len()));
        dict.extend(dict_int(private_at));
        dict.push(18);
        index(&[dict])
    };
    let empty = index(&[]);
    let charstrings_at = header.len() + name.len() + top(0, 0).len() + 2 * empty.len();
    let private_at = charstrings_at + charstrings.len();
    let mut cff = header;
    cff.extend(name);
    cff.extend(top(charstrings_at, private_at));
    cff.extend(&empty);
    cff.extend(&empty);
    cff.extend(charstrings);
    cff.extend(private);

    let base: &[u8] = include_bytes!("../../../tests/fixtures/fallback-only.ttf");
    let mut tables = tables_of(base);
    tables.retain(|(tag, _)| !matches!(tag, b"glyf" | b"loca" | b"GDEF" | b"GSUB" | b"GPOS"));
    for (tag, data) in &mut tables {
        match &*tag {
            b"maxp" => {
                *data = [
                    0x0000_5000u32.to_be_bytes().to_vec(),
                    3u16.to_be_bytes().to_vec(),
                ]
                .concat()
            }
            b"hhea" => data[34..36].copy_from_slice(&3u16.to_be_bytes()),
            b"hmtx" => {
                *data = [(1_344u16, 0i16), (0, 0), (1_000, 100)]
                    .iter()
                    .flat_map(|(advance, bearing)| [advance.to_be_bytes(), bearing.to_be_bytes()])
                    .flatten()
                    .collect();
            }
            b"cmap" => {
                *data = cmap_of(
                    &[format12(&[
                        (0x300, 0x300, 1),
                        (0x301, 0x301, 2),
                        (0xF0010, 0xF0010, 2),
                    ])],
                    &[(3, 10, 0)],
                );
            }
            _ => {}
        }
    }
    tables.push((*b"CFF ", cff));
    let mut font = assemble(tables);
    font[0..4].copy_from_slice(b"OTTO");
    font
}

/// Inter Regular with the seven bytes changed that a fuzzing seed found: in debug builds
/// the previous shaper, rustybuzz, panicked on it with an arithmetic overflow.
pub(super) fn bit_flipped_inter() -> Vec<u8> {
    const CHANGES: [(usize, u8); 7] = [
        (959, 11),
        (4201, 148),
        (11280, 109),
        (14203, 16),
        (24820, 97),
        (25908, 26),
        (28908, 225),
    ];
    let mut bytes = font_bytes(BundledFont::Inter, Weight::Regular).to_vec();
    for (offset, value) in CHANGES {
        bytes[offset] = value;
    }
    bytes
}

pub(super) fn assert_no_bundled_font_has(text: &str) {
    let lib = FontLibrary::with_fallbacks(vec![]);
    for c in text.chars() {
        for face in lib.all_bundled_parsed() {
            assert!(face.glyph(c).is_none(), "a bundled font has {c:?}");
        }
    }
}

#[test]
fn fixture_fonts_parse_with_the_requested_properties() {
    let bytes = fixture_font("Test Family", 600, true, &[0xF0000, 0xF0001, 0xF0005]);
    let face = Face::parse(&bytes, 0).unwrap();
    assert!(face.glyph('\u{F0005}').is_some());
    assert!(face.glyph('\u{F0002}').is_none());
    let mut db = fontdb::Database::new();
    db.load_font_data(bytes);
    let info = db.faces().next().unwrap();
    assert_eq!(info.families[0].0, "Test Family");
    assert_eq!(info.post_script_name, "TestFamily-600Italic");
    assert_eq!(info.weight.0, 600);
    assert_ne!(info.style, fontdb::Style::Normal);
}
