//! The font files bundled with the crate.
//!
//! Five OFL-licensed families are embedded with `include_bytes!`, each in two weights: Source
//! Serif 4, IBM Plex Sans, Geist, Inter and JetBrains Mono. Families that declare a Reserved
//! Font Name are shipped unmodified; the others are subset. Each family's license text ships
//! with its fonts, and `fonts/MANIFEST.md` records where every file came from.
//!
//! # Examples
//!
//! ```
//! use umber_core::fonts::{Font, Weight, font_bytes};
//!
//! assert!(!font_bytes(Font::Inter, Weight::Regular).is_empty());
//! ```

/// A bundled font family.
///
/// # Examples
///
/// ```
/// use umber_core::Font;
///
/// assert_eq!(Font::ALL.len(), 5);
/// assert_eq!(Font::ALL[3], Font::Inter);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Font {
    /// Source Serif 4.
    SourceSerif4,
    /// IBM Plex Sans.
    IbmPlexSans,
    /// Geist.
    Geist,
    /// Inter.
    Inter,
    /// JetBrains Mono.
    JetBrainsMono,
}

impl Font {
    /// Every bundled family.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Font;
    ///
    /// assert!(Font::ALL.contains(&Font::Geist));
    /// ```
    pub const ALL: [Font; 5] = [
        Font::SourceSerif4,
        Font::IbmPlexSans,
        Font::Geist,
        Font::Inter,
        Font::JetBrainsMono,
    ];
}

/// A bundled font weight.
///
/// # Examples
///
/// ```
/// use umber_core::Weight;
///
/// assert_eq!(Weight::ALL, [Weight::Regular, Weight::SemiBold]);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weight {
    /// Regular (400).
    Regular,
    /// SemiBold (600).
    SemiBold,
}

impl Weight {
    /// Every bundled weight.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Weight;
    ///
    /// assert!(Weight::ALL.contains(&Weight::SemiBold));
    /// ```
    pub const ALL: [Weight; 2] = [Weight::Regular, Weight::SemiBold];
}

/// Code points (inclusive ranges) that every bundled font covers.
///
/// This is Basic Latin and Latin-1 Supplement (without U+00AD, the soft hyphen), the en and em
/// dashes, curly quotes, bullet, ellipsis, per mille, single guillemets, the euro sign and the
/// minus sign.
///
/// # Examples
///
/// ```
/// use umber_core::fonts::REQUIRED_CODEPOINTS;
///
/// assert!(REQUIRED_CODEPOINTS.iter().any(|&(lo, hi)| lo <= 0x2212 && 0x2212 <= hi));
/// ```
pub const REQUIRED_CODEPOINTS: &[(u32, u32)] = &[
    (0x0020, 0x007E),
    (0x00A0, 0x00AC),
    (0x00AE, 0x00FF),
    (0x2013, 0x2014),
    (0x2018, 0x2019),
    (0x201C, 0x201D),
    (0x2022, 0x2022),
    (0x2026, 0x2026),
    (0x2030, 0x2030),
    (0x2039, 0x203A),
    (0x20AC, 0x20AC),
    (0x2212, 0x2212),
];

/// Code points (inclusive ranges) that each bundled font keeps if it has them.
///
/// At least one bundled font covers every code point listed here; text layout falls back to the
/// other bundled fonts for the rest. This is Latin Extended-A plus less common punctuation.
///
/// # Examples
///
/// ```
/// use umber_core::fonts::EXTENDED_CODEPOINTS;
///
/// assert!(EXTENDED_CODEPOINTS.iter().any(|&(lo, hi)| lo <= 0x0141 && 0x0141 <= hi));
/// ```
pub const EXTENDED_CODEPOINTS: &[(u32, u32)] = &[
    (0x0100, 0x017F),
    (0x2010, 0x2012),
    (0x2015, 0x2017),
    (0x201A, 0x201B),
    (0x201E, 0x2021),
    (0x2023, 0x2025),
    (0x2027, 0x2027),
    (0x2031, 0x2038),
];

/// A private-use code point that no bundled font maps.
///
/// Tests use it with `tests/fixtures/fallback-only.ttf`, a font whose only glyph is mapped to
/// this code point. Text containing it can only be drawn by a fallback font, which exercises the
/// fallback path even though every bundled family is complete enough for ordinary text.
///
/// # Examples
///
/// ```
/// use umber_core::fonts::FALLBACK_TEST_CODEPOINT;
///
/// assert!(('\u{E000}'..='\u{F8FF}').contains(&FALLBACK_TEST_CODEPOINT));
/// ```
pub const FALLBACK_TEST_CODEPOINT: char = '\u{E000}';

/// The TrueType bytes of a bundled font.
///
/// # Examples
///
/// ```
/// use umber_core::fonts::font_bytes;
/// use umber_core::{Font, Weight};
///
/// let bytes = font_bytes(Font::Geist, Weight::SemiBold);
/// assert!(bytes.len() > 1000);
/// ```
#[must_use]
pub fn font_bytes(font: Font, weight: Weight) -> &'static [u8] {
    match (font, weight) {
        (Font::SourceSerif4, Weight::Regular) => {
            include_bytes!("../fonts/SourceSerif4-Regular.ttf")
        }
        (Font::SourceSerif4, Weight::SemiBold) => {
            include_bytes!("../fonts/SourceSerif4-SemiBold.ttf")
        }
        (Font::IbmPlexSans, Weight::Regular) => include_bytes!("../fonts/IBMPlexSans-Regular.ttf"),
        (Font::IbmPlexSans, Weight::SemiBold) => {
            include_bytes!("../fonts/IBMPlexSans-SemiBold.ttf")
        }
        (Font::Geist, Weight::Regular) => include_bytes!("../fonts/Geist-Regular.ttf"),
        (Font::Geist, Weight::SemiBold) => include_bytes!("../fonts/Geist-SemiBold.ttf"),
        (Font::Inter, Weight::Regular) => include_bytes!("../fonts/Inter-Regular.ttf"),
        (Font::Inter, Weight::SemiBold) => include_bytes!("../fonts/Inter-SemiBold.ttf"),
        (Font::JetBrainsMono, Weight::Regular) => {
            include_bytes!("../fonts/JetBrainsMono-Regular.ttf")
        }
        (Font::JetBrainsMono, Weight::SemiBold) => {
            include_bytes!("../fonts/JetBrainsMono-SemiBold.ttf")
        }
    }
}

/// The SIL Open Font License text that ships with a family.
///
/// # Examples
///
/// ```
/// use umber_core::fonts::license_text;
/// use umber_core::Font;
///
/// assert!(license_text(Font::Inter).contains("SIL OPEN FONT LICENSE"));
/// ```
#[must_use]
pub fn license_text(font: Font) -> &'static str {
    match font {
        Font::SourceSerif4 => include_str!("../fonts/LICENSES/SourceSerif4-OFL.txt"),
        Font::IbmPlexSans => include_str!("../fonts/LICENSES/IBMPlexSans-OFL.txt"),
        Font::Geist => include_str!("../fonts/LICENSES/Geist-OFL.txt"),
        Font::Inter => include_str!("../fonts/LICENSES/Inter-OFL.txt"),
        Font::JetBrainsMono => include_str!("../fonts/LICENSES/JetBrainsMono-OFL.txt"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use read_fonts::{FontRef, TableProvider};
    use skrifa::MetadataProvider;

    fn codepoints(ranges: &[(u32, u32)]) -> impl Iterator<Item = u32> + '_ {
        ranges.iter().flat_map(|&(lo, hi)| lo..=hi)
    }

    #[test]
    fn every_font_parses_and_covers_required_codepoints() {
        for font in Font::ALL {
            for weight in Weight::ALL {
                let face = FontRef::new(font_bytes(font, weight))
                    .unwrap_or_else(|e| panic!("{font:?} {weight:?}: {e}"));
                assert!(face.head().expect("head table").units_per_em() > 0);
                let charmap = face.charmap();
                for cp in codepoints(REQUIRED_CODEPOINTS) {
                    let Some(ch) = char::from_u32(cp) else {
                        continue;
                    };
                    assert!(
                        charmap.map(ch).is_some(),
                        "{font:?} {weight:?} lacks U+{cp:04X}"
                    );
                }
            }
        }
    }

    #[test]
    fn extended_codepoints_are_covered_by_some_bundled_font() {
        let charmaps: Vec<_> = Font::ALL
            .iter()
            .flat_map(|&f| Weight::ALL.iter().map(move |&w| (f, w)))
            .map(|(f, w)| {
                FontRef::new(font_bytes(f, w))
                    .unwrap_or_else(|e| panic!("{f:?} {w:?}: {e}"))
                    .charmap()
            })
            .collect();
        for cp in codepoints(EXTENDED_CODEPOINTS) {
            let Some(ch) = char::from_u32(cp) else {
                continue;
            };
            assert!(
                charmaps.iter().any(|charmap| charmap.map(ch).is_some()),
                "no bundled font has U+{cp:04X}"
            );
        }
    }

    #[test]
    fn fallback_codepoint_is_in_no_bundled_font_but_in_the_fixture() {
        for font in Font::ALL {
            for weight in Weight::ALL {
                let face = FontRef::new(font_bytes(font, weight))
                    .unwrap_or_else(|e| panic!("{font:?} {weight:?}: {e}"));
                assert!(
                    face.charmap().map(FALLBACK_TEST_CODEPOINT).is_none(),
                    "{font:?} {weight:?} maps {FALLBACK_TEST_CODEPOINT:?}"
                );
            }
        }
        let fixture = include_bytes!("../tests/fixtures/fallback-only.ttf");
        let face = FontRef::new(fixture).expect("fixture parses");
        assert!(face.charmap().map(FALLBACK_TEST_CODEPOINT).is_some());
    }

    #[test]
    fn weights_are_distinct_files() {
        for font in Font::ALL {
            assert_ne!(
                font_bytes(font, Weight::Regular),
                font_bytes(font, Weight::SemiBold)
            );
        }
    }

    #[test]
    fn every_family_ships_its_license() {
        for font in Font::ALL {
            let text = license_text(font);
            assert!(text.contains("SIL OPEN FONT LICENSE"), "{font:?}");
        }
    }

    #[test]
    fn total_font_size_is_within_budget() {
        let total: usize = Font::ALL
            .iter()
            .flat_map(|f| Weight::ALL.iter().map(move |w| font_bytes(*f, *w).len()))
            .sum();
        assert!(total <= 3 * 1024 * 1024, "fonts total {total} bytes");
    }
}
