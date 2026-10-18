//! Fonts: the families text is set in, and the font files bundled with the crate.
//!
//! A [`Font`] is a value naming the family a theme sets its titles or body text in. Five
//! OFL-licensed families are bundled ([`BundledFont`]), embedded with `include_bytes!`, each in
//! two weights: Source Serif 4, IBM Plex Sans, Geist, Inter and JetBrains Mono. Families that
//! declare a Reserved Font Name are shipped unmodified; the others are subset. Each family's
//! license text ships with its fonts, and `fonts/MANIFEST.md` records where every file came
//! from.
//!
//! # Examples
//!
//! ```
//! use umber_core::fonts::{BundledFont, Font, Weight, font_bytes};
//!
//! assert_eq!(Font::INTER, Font::Bundled(BundledFont::Inter));
//! assert!(!font_bytes(BundledFont::Inter, Weight::Regular).is_empty());
//! ```

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::text::OwnedFace;
use crate::{Error, Result};

/// A font family bundled with the crate.
///
/// # Examples
///
/// ```
/// use umber_core::BundledFont;
///
/// assert_eq!(BundledFont::ALL.len(), 5);
/// assert_eq!(BundledFont::ALL[3], BundledFont::Inter);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BundledFont {
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

impl BundledFont {
    /// Every bundled family.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::BundledFont;
    ///
    /// assert!(BundledFont::ALL.contains(&BundledFont::Geist));
    /// ```
    pub const ALL: [BundledFont; 5] = [
        BundledFont::SourceSerif4,
        BundledFont::IbmPlexSans,
        BundledFont::Geist,
        BundledFont::Inter,
        BundledFont::JetBrainsMono,
    ];

    /// The family name used in SVG `font-family`.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::BundledFont;
    ///
    /// assert_eq!(BundledFont::SourceSerif4.family_name(), "Source Serif 4");
    /// ```
    #[must_use]
    pub fn family_name(self) -> &'static str {
        match self {
            BundledFont::SourceSerif4 => "Source Serif 4",
            BundledFont::IbmPlexSans => "IBM Plex Sans",
            BundledFont::Geist => "Geist",
            BundledFont::Inter => "Inter",
            BundledFont::JetBrainsMono => "JetBrains Mono",
        }
    }

    /// The CSS generic family to fall back to: `serif` for Source Serif 4, `monospace` for
    /// JetBrains Mono, and `sans-serif` for the rest.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::BundledFont;
    ///
    /// assert_eq!(BundledFont::JetBrainsMono.generic_family(), "monospace");
    /// ```
    #[must_use]
    pub fn generic_family(self) -> &'static str {
        match self {
            BundledFont::SourceSerif4 => "serif",
            BundledFont::JetBrainsMono => "monospace",
            BundledFont::IbmPlexSans | BundledFont::Geist | BundledFont::Inter => "sans-serif",
        }
    }
}

/// A font family that text is set in, such as a theme's title or body font.
///
/// Fonts are values: two fonts are equal when they name the same family. The bundled families
/// have one constant each.
///
/// # Examples
///
/// ```
/// use umber_core::{BundledFont, Font, Theme};
///
/// let theme = Theme::pigment().with_title_font(Font::INTER)?;
/// assert_eq!(theme.title_font(), &Font::Bundled(BundledFont::Inter));
/// assert_eq!(theme.title_font().family_name(), "Inter");
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Font {
    /// A family bundled with the crate.
    Bundled(BundledFont),
    /// Validated regular and optional bold files, identified by bytes and resolved family name.
    File(Arc<FontFiles>),
    /// An installed family, with the selected faces kept in memory.
    System(Arc<SystemFamily>),
}

/// The immutable files of a font made with [`Font::from_files`].
///
/// Equality uses the regular and optional bold bytes, the resolved family name and the bold
/// fallback setting. Paths do not affect equality, except for the fallback name used when the
/// regular file has no name.
///
/// # Examples
///
/// ```
/// use umber_core::{BundledFont, Font, Weight};
/// use umber_core::fonts::font_bytes;
/// let font = Font::from_files(font_bytes(BundledFont::Inter, Weight::Regular).to_vec(), None, "Inter".into())?;
/// assert!(matches!(font, Font::File(_)));
/// # Ok::<(), umber_core::Error>(())
/// ```
#[derive(Debug, Clone)]
pub struct FontFiles {
    regular: OwnedFace,
    bold: Option<OwnedFace>,
    fingerprint: (u64, Option<u64>),
    bold_fallback: bool,
}

fn same_bytes(left: &Arc<Vec<u8>>, right: &Arc<Vec<u8>>) -> bool {
    Arc::ptr_eq(left, right) || (left.len() == right.len() && left.as_slice() == right.as_slice())
}

fn fingerprint(bytes: &[u8]) -> u64 {
    let mut hash = DefaultHasher::new();
    hash.write(bytes);
    hash.finish()
}

impl PartialEq for FontFiles {
    fn eq(&self, other: &Self) -> bool {
        self.bold_fallback == other.bold_fallback
            && self.regular.family == other.regular.family
            && same_bytes(&self.regular.bytes, &other.regular.bytes)
            && match (&self.bold, &other.bold) {
                (Some(left), Some(right)) => same_bytes(&left.bytes, &right.bytes),
                (None, None) => true,
                _ => false,
            }
    }
}
impl Eq for FontFiles {}
impl Hash for FontFiles {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.regular.family.hash(state);
        self.regular.bytes.len().hash(state);
        self.bold.as_ref().map(|face| face.bytes.len()).hash(state);
        self.fingerprint.hash(state);
        self.bold_fallback.hash(state);
    }
}

impl FontFiles {
    /// The stored bytes for a weight, without copying or reopening the file.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{BundledFont, Font, Weight};
    /// use umber_core::fonts::font_bytes;
    /// let bytes = font_bytes(BundledFont::Inter, Weight::Regular);
    /// let Font::File(files) = Font::from_files(bytes.to_vec(), None, "Inter".into())? else {
    ///     unreachable!()
    /// };
    /// assert_eq!(files.bytes(Weight::Regular), Some(bytes));
    /// assert_eq!(files.bytes(Weight::SemiBold), None);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    #[must_use]
    pub fn bytes(&self, weight: Weight) -> Option<&[u8]> {
        match weight {
            Weight::Regular => Some(&self.regular.bytes),
            Weight::SemiBold => self.bold.as_ref().map(|face| face.bytes.as_slice()),
        }
    }
}

/// An installed family and immutable copies of its selected regular and bold faces.
///
/// Equality uses the canonical family name and the bold fallback setting, independently of
/// the spelling used to find the family.
///
/// # Examples
///
/// ```
/// use umber_core::Font;
/// if let Ok(font) = Font::system("DejaVu Sans") {
///     assert!(matches!(font, Font::System(_)));
/// }
/// ```
#[derive(Debug, Clone)]
pub struct SystemFamily {
    name: String,
    regular: OwnedFace,
    bold: Option<OwnedFace>,
    variable_only: bool,
    bold_fallback: bool,
}

impl SystemFamily {
    pub(crate) fn new(
        name: String,
        regular: OwnedFace,
        bold: Option<OwnedFace>,
        variable_only: bool,
    ) -> Self {
        SystemFamily {
            name,
            regular,
            bold,
            variable_only,
            bold_fallback: false,
        }
    }
}
impl PartialEq for SystemFamily {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.bold_fallback == other.bold_fallback
    }
}
impl Eq for SystemFamily {}
impl Hash for SystemFamily {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.bold_fallback.hash(state);
    }
}

impl Font {
    /// Source Serif 4, a bundled serif face.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Font;
    ///
    /// assert_eq!(Font::SOURCE_SERIF_4.family_name(), "Source Serif 4");
    /// ```
    pub const SOURCE_SERIF_4: Font = Font::Bundled(BundledFont::SourceSerif4);

    /// IBM Plex Sans, a bundled sans-serif face.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Font;
    ///
    /// assert_eq!(Font::IBM_PLEX_SANS.family_name(), "IBM Plex Sans");
    /// ```
    pub const IBM_PLEX_SANS: Font = Font::Bundled(BundledFont::IbmPlexSans);

    /// Geist, a bundled sans-serif face.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Font;
    ///
    /// assert_eq!(Font::GEIST.family_name(), "Geist");
    /// ```
    pub const GEIST: Font = Font::Bundled(BundledFont::Geist);

    /// Inter, a bundled sans-serif face.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Font;
    ///
    /// assert_eq!(Font::INTER.family_name(), "Inter");
    /// ```
    pub const INTER: Font = Font::Bundled(BundledFont::Inter);

    /// JetBrains Mono, a bundled monospaced face.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Font;
    ///
    /// assert_eq!(Font::JETBRAINS_MONO.family_name(), "JetBrains Mono");
    /// ```
    pub const JETBRAINS_MONO: Font = Font::Bundled(BundledFont::JetBrainsMono);

    /// Parses a regular font file and an optional bold file into an immutable font.
    ///
    /// The bold file fills the SemiBold slot; no synthetic bold is made. Variable fonts use
    /// their default instance. `name` is used when the font has no family name. Font licensing
    /// is the caller's responsibility. Editable SVG viewers need the font installed.
    ///
    /// # Errors
    ///
    /// `Error::Data` if either file is damaged, is not a TrueType or OpenType font, or is a
    /// collection (`.ttc`). A missing bold face is allowed here and fails only when laying out
    /// bold text, unless [`Font::with_bold_fallback`] allows the regular face instead.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{BundledFont, Font, Weight};
    /// use umber_core::fonts::font_bytes;
    /// let font = Font::from_files(font_bytes(BundledFont::Inter, Weight::Regular).to_vec(), None, "Inter".into())?;
    /// assert_eq!(font.family_name(), "Inter");
    /// assert!(!font.has_face(Weight::SemiBold));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn from_files(regular: Vec<u8>, bold: Option<Vec<u8>>, name: String) -> Result<Font> {
        let regular = parse_file(regular, "regular", &name)?;
        let bold = bold
            .map(|bytes| {
                if bytes == *regular.bytes {
                    Ok(regular.clone())
                } else {
                    parse_file(bytes, "bold", &name)
                }
            })
            .transpose()?;
        // Validate both faces before traversing their bytes for hashing.
        let regular_hash = fingerprint(&regular.bytes);
        let bold_hash = bold.as_ref().map(|face| {
            if Arc::ptr_eq(&regular.bytes, &face.bytes) {
                regular_hash
            } else {
                fingerprint(&face.bytes)
            }
        });
        Ok(Font::File(Arc::new(FontFiles {
            regular,
            bold,
            fingerprint: (regular_hash, bold_hash),
            bold_fallback: false,
        })))
    }

    /// Finds an installed family and keeps its selected faces in memory.
    ///
    /// Every call scans the installed fonts; there is no cache. Family matching is
    /// case-insensitive; legacy names are tried only when typographic names do not match.
    /// The regular face prefers upright,
    /// normal-width faces, then the closest weight to 400. Bold requires a separate upright
    /// static face at least weight 600, preferring normal width, then the closest weight to 600.
    /// Variable fonts use their default instance, not a bold variation. Families installed only
    /// as variable fonts have no bold face here. [`Font::with_bold_fallback`] allows them to
    /// draw bold text in the regular face. Font licensing is the caller's responsibility.
    ///
    /// # Errors
    ///
    /// `Error::Usage` if the family is not installed, with the closest installed names in the
    /// message, or if a selected font file cannot be read or parsed.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Font;
    /// if let Ok(font) = Font::system("DejaVu Sans") {
    ///     assert_eq!(font.family_name(), "DejaVu Sans");
    /// }
    /// ```
    pub fn system(family: &str) -> Result<Font> {
        crate::text::find_family(family).map(|family| Font::System(Arc::new(family)))
    }

    /// Sets whether a custom font without bold draws bold text in its regular face.
    ///
    /// No synthetic bold is made. An existing system bold face is always used; bundled fonts
    /// are unchanged. This setting is part of a custom font's equality and hash, but does not
    /// change [`Font::has_face`].
    ///
    /// # Errors
    ///
    /// `Error::Usage` if `value` is true and a file font has an explicit bold face.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{BundledFont, Font, Weight};
    /// use umber_core::fonts::font_bytes;
    /// let font = Font::from_files(font_bytes(BundledFont::Inter, Weight::Regular).to_vec(), None, "Inter".into())?
    ///     .with_bold_fallback(true)?;
    /// assert!(font.bold_fallback());
    /// assert!(!font.has_face(Weight::SemiBold));
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn with_bold_fallback(mut self, value: bool) -> Result<Font> {
        match &mut self {
            Font::Bundled(_) => {}
            Font::File(font) => {
                check_file_bold_fallback(font.bold.is_some(), value)?;
                Arc::make_mut(font).bold_fallback = value;
            }
            Font::System(font) => Arc::make_mut(font).bold_fallback = value,
        }
        Ok(self)
    }

    /// Whether a custom font may draw bold text in its regular face when bold is absent.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Font;
    /// assert!(!Font::INTER.bold_fallback());
    /// ```
    #[must_use]
    pub fn bold_fallback(&self) -> bool {
        match self {
            Font::Bundled(_) => false,
            Font::File(font) => font.bold_fallback,
            Font::System(font) => font.bold_fallback,
        }
    }

    /// The actual weight for shaping and editable output, without inventing a bold face.
    pub(crate) fn resolved_weight(&self, weight: Weight) -> Option<Weight> {
        if self.has_face(weight) {
            Some(weight)
        } else if weight == Weight::SemiBold && self.bold_fallback() {
            Some(Weight::Regular)
        } else {
            None
        }
    }

    /// Whether a missing bold face comes from an installed family containing only variable files.
    pub(crate) fn is_variable_only_system(&self) -> bool {
        matches!(self, Font::System(family) if family.variable_only)
    }

    /// The owned face of a custom font; bundled faces live in the font library instead.
    pub(crate) fn owned_face(&self, weight: Weight) -> Option<&OwnedFace> {
        let (regular, bold) = match self {
            Font::Bundled(_) => return None,
            Font::File(font) => (&font.regular, &font.bold),
            Font::System(font) => (&font.regular, &font.bold),
        };
        match weight {
            Weight::Regular => Some(regular),
            Weight::SemiBold => bold.as_ref(),
        }
    }

    /// The family name, as SVG `font-family` writes it.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Font;
    ///
    /// assert_eq!(Font::IBM_PLEX_SANS.family_name(), "IBM Plex Sans");
    /// ```
    #[must_use]
    pub fn family_name(&self) -> &str {
        match self {
            Font::Bundled(font) => font.family_name(),
            Font::File(font) => &font.regular.family,
            Font::System(font) => &font.name,
        }
    }

    /// Whether the font has a face for `weight`. Every bundled font has a face for every
    /// weight.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::{Font, Weight};
    ///
    /// assert!(Font::INTER.has_face(Weight::SemiBold));
    /// ```
    #[must_use]
    pub fn has_face(&self, weight: Weight) -> bool {
        match self {
            Font::Bundled(_) => true,
            Font::File(_) | Font::System(_) => self.owned_face(weight).is_some(),
        }
    }

    /// The CSS generic family that SVG `font-family` falls back to.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::Font;
    ///
    /// assert_eq!(Font::SOURCE_SERIF_4.generic_family(), "serif");
    /// ```
    #[must_use]
    pub fn generic_family(&self) -> &'static str {
        match self {
            Font::Bundled(font) => font.generic_family(),
            Font::File(font) => font.regular.generic,
            Font::System(font) => font.regular.generic,
        }
    }
}

/// Checks a file font's bold options before its files are read or parsed.
///
/// # Errors
///
/// `Error::Usage` if both an explicit bold face and regular-face fallback are requested.
///
/// # Examples
///
/// ```
/// use umber_core::fonts::check_file_bold_fallback;
/// assert!(check_file_bold_fallback(true, true).is_err());
/// assert!(check_file_bold_fallback(false, true).is_ok());
/// ```
pub fn check_file_bold_fallback(has_bold: bool, bold_fallback: bool) -> Result<()> {
    if has_bold && bold_fallback {
        return Err(Error::usage(
            "Font.from_file()",
            "bold_fallback is True with bold given; \
             the fallback applies only to a font without a bold face.",
        )
        .with_parameter("bold_fallback")
        .with_hint("drop bold_fallback, or drop bold."));
    }
    Ok(())
}

fn parse_file(bytes: Vec<u8>, parameter: &'static str, name: &str) -> Result<OwnedFace> {
    let hint = if bytes.starts_with(b"ttcf") {
        "extract one face from the collection, or use Font.system()."
    } else {
        return OwnedFace::parse(Arc::new(bytes), 0, name)
            .ok_or_else(|| invalid_file(parameter, "pass a .ttf or .otf file."));
    };
    Err(invalid_file(parameter, hint))
}

fn invalid_file(parameter: &'static str, hint: &'static str) -> Error {
    Error::data(
        "Font.from_file()",
        format!("{parameter} is not a valid font; it must be a TrueType or OpenType font."),
    )
    .with_parameter(parameter)
    .with_hint(hint)
}

/// A font weight.
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

    /// The position of this weight in [`Weight::ALL`]: 0 for Regular, 1 for SemiBold. Tables
    /// with one entry per weight are indexed by it.
    pub(crate) fn slot(self) -> usize {
        match self {
            Weight::Regular => 0,
            Weight::SemiBold => 1,
        }
    }

    /// The numeric weight, as CSS `font-weight` and the OpenType `usWeightClass` use it: 400
    /// for Regular, 600 for SemiBold.
    pub(crate) fn numeric(self) -> u16 {
        match self {
            Weight::Regular => 400,
            Weight::SemiBold => 600,
        }
    }
}

/// Code points (inclusive ranges) that every bundled font covers.
///
/// This is Basic Latin and Latin-1 Supplement (without U+00AD, the soft hyphen), the en and em
/// dashes, curly quotes, bullet, ellipsis, per mille, single guillemets, the euro sign and the
/// minus sign. `scripts/fonts/build_fonts.py` subsets with the same ranges, and the tests check
/// every bundled file against them.
#[cfg(test)]
pub(crate) const REQUIRED_CODEPOINTS: &[(u32, u32)] = &[
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
/// `scripts/fonts/build_fonts.py` keeps the same ranges.
#[cfg(test)]
pub(crate) const EXTENDED_CODEPOINTS: &[(u32, u32)] = &[
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
/// `scripts/fonts/build_fonts.py` reads this declaration to build that font, so it keeps this
/// path and form.
#[cfg(test)]
pub(crate) const FALLBACK_TEST_CODEPOINT: char = '\u{E000}';

/// The TrueType bytes of a bundled font.
///
/// # Examples
///
/// ```
/// use umber_core::fonts::font_bytes;
/// use umber_core::{BundledFont, Weight};
///
/// let bytes = font_bytes(BundledFont::Geist, Weight::SemiBold);
/// assert!(bytes.len() > 1000);
/// ```
#[must_use]
pub fn font_bytes(font: BundledFont, weight: Weight) -> &'static [u8] {
    match (font, weight) {
        (BundledFont::SourceSerif4, Weight::Regular) => {
            include_bytes!("../fonts/SourceSerif4-Regular.ttf")
        }
        (BundledFont::SourceSerif4, Weight::SemiBold) => {
            include_bytes!("../fonts/SourceSerif4-SemiBold.ttf")
        }
        (BundledFont::IbmPlexSans, Weight::Regular) => {
            include_bytes!("../fonts/IBMPlexSans-Regular.ttf")
        }
        (BundledFont::IbmPlexSans, Weight::SemiBold) => {
            include_bytes!("../fonts/IBMPlexSans-SemiBold.ttf")
        }
        (BundledFont::Geist, Weight::Regular) => include_bytes!("../fonts/Geist-Regular.ttf"),
        (BundledFont::Geist, Weight::SemiBold) => include_bytes!("../fonts/Geist-SemiBold.ttf"),
        (BundledFont::Inter, Weight::Regular) => include_bytes!("../fonts/Inter-Regular.ttf"),
        (BundledFont::Inter, Weight::SemiBold) => include_bytes!("../fonts/Inter-SemiBold.ttf"),
        (BundledFont::JetBrainsMono, Weight::Regular) => {
            include_bytes!("../fonts/JetBrainsMono-Regular.ttf")
        }
        (BundledFont::JetBrainsMono, Weight::SemiBold) => {
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
/// use umber_core::BundledFont;
///
/// assert!(license_text(BundledFont::Inter).contains("SIL OPEN FONT LICENSE"));
/// ```
#[must_use]
pub fn license_text(font: BundledFont) -> &'static str {
    match font {
        BundledFont::SourceSerif4 => include_str!("../fonts/LICENSES/SourceSerif4-OFL.txt"),
        BundledFont::IbmPlexSans => include_str!("../fonts/LICENSES/IBMPlexSans-OFL.txt"),
        BundledFont::Geist => include_str!("../fonts/LICENSES/Geist-OFL.txt"),
        BundledFont::Inter => include_str!("../fonts/LICENSES/Inter-OFL.txt"),
        BundledFont::JetBrainsMono => include_str!("../fonts/LICENSES/JetBrainsMono-OFL.txt"),
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
    fn fingerprint_collisions_do_not_make_different_files_equal() {
        let make = |suffix| {
            let mut bytes = font_bytes(BundledFont::Inter, Weight::Regular).to_vec();
            bytes.push(suffix);
            let Font::File(files) = Font::from_files(bytes, None, "Inter".into()).unwrap() else {
                unreachable!()
            };
            files
        };
        let first = make(1);
        let mut second = make(2);
        Arc::get_mut(&mut second).unwrap().fingerprint = first.fingerprint;
        assert_ne!(first, second);
        assert_eq!(first, make(1));
        assert_eq!(first, Arc::clone(&first));
    }

    #[test]
    fn every_font_parses_and_covers_required_codepoints() {
        for font in BundledFont::ALL {
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
        let charmaps: Vec<_> = BundledFont::ALL
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
        for font in BundledFont::ALL {
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
    fn coverage_constants_hold_their_documented_code_points() {
        let covers =
            |ranges: &[(u32, u32)], cp: u32| ranges.iter().any(|&(lo, hi)| lo <= cp && cp <= hi);
        assert!(covers(REQUIRED_CODEPOINTS, 0x2212));
        assert!(!covers(REQUIRED_CODEPOINTS, 0x00AD));
        assert!(covers(EXTENDED_CODEPOINTS, 0x0141));
        assert!(('\u{E000}'..='\u{F8FF}').contains(&FALLBACK_TEST_CODEPOINT));
    }

    #[test]
    fn every_font_and_weight_maps_to_its_slot_and_numeric_weight() {
        let mut seen = 0;
        for font in BundledFont::ALL {
            for (index, weight) in Weight::ALL.into_iter().enumerate() {
                let (slot, numeric) = match weight {
                    Weight::Regular => (0, 400),
                    Weight::SemiBold => (1, 600),
                };
                assert_eq!(weight.slot(), slot, "{font:?} {weight:?}");
                assert_eq!(weight.slot(), index, "{font:?} {weight:?}");
                assert_eq!(weight.numeric(), numeric, "{font:?} {weight:?}");
                let class = FontRef::new(font_bytes(font, weight))
                    .unwrap_or_else(|e| panic!("{font:?} {weight:?}: {e}"))
                    .os2()
                    .expect("OS/2 table")
                    .us_weight_class();
                assert_eq!(class, numeric, "{font:?} {weight:?}");
                seen += 1;
            }
        }
        assert_eq!(seen, 10);
    }

    #[test]
    fn family_names_are_the_published_names() {
        let expected = [
            (BundledFont::SourceSerif4, "Source Serif 4", "serif"),
            (BundledFont::IbmPlexSans, "IBM Plex Sans", "sans-serif"),
            (BundledFont::Geist, "Geist", "sans-serif"),
            (BundledFont::Inter, "Inter", "sans-serif"),
            (BundledFont::JetBrainsMono, "JetBrains Mono", "monospace"),
        ];
        assert_eq!(expected.len(), BundledFont::ALL.len());
        for (font, name, generic) in expected {
            assert_eq!(font.family_name(), name);
            assert_eq!(font.generic_family(), generic);
        }
    }

    #[test]
    fn bundled_constants_are_the_bundled_families() {
        let constants = [
            Font::SOURCE_SERIF_4,
            Font::IBM_PLEX_SANS,
            Font::GEIST,
            Font::INTER,
            Font::JETBRAINS_MONO,
        ];
        assert_eq!(constants.len(), BundledFont::ALL.len());
        for (constant, bundled) in constants.iter().zip(BundledFont::ALL) {
            assert_eq!(*constant, Font::Bundled(bundled));
            assert_eq!(constant.family_name(), bundled.family_name());
            assert_eq!(constant.generic_family(), bundled.generic_family());
            assert!(
                Weight::ALL
                    .into_iter()
                    .all(|weight| constant.has_face(weight))
            );
        }
        for (i, a) in constants.iter().enumerate() {
            for (j, b) in constants.iter().enumerate() {
                assert_eq!(a == b, i == j, "{a:?} {b:?}");
            }
        }
    }

    #[test]
    fn weights_are_distinct_files() {
        for font in BundledFont::ALL {
            assert_ne!(
                font_bytes(font, Weight::Regular),
                font_bytes(font, Weight::SemiBold)
            );
        }
    }

    #[test]
    fn every_family_ships_its_license() {
        for font in BundledFont::ALL {
            let text = license_text(font);
            assert!(text.contains("SIL OPEN FONT LICENSE"), "{font:?}");
        }
    }

    #[test]
    fn total_font_size_is_within_budget() {
        let total: usize = BundledFont::ALL
            .iter()
            .flat_map(|f| Weight::ALL.iter().map(move |w| font_bytes(*f, *w).len()))
            .sum();
        assert!(total <= 3 * 1024 * 1024, "fonts total {total} bytes");
    }
}
