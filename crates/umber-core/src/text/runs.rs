//! Splitting text into runs that are each drawn with one font, and grouping runs by font.

use std::collections::HashMap;
use std::ops::Range;

use super::font::FontKey;
use super::marks;

/// A piece of the input text drawn with one font.
#[derive(Debug, Clone)]
pub(super) struct TextRun {
    pub(super) font: FontKey,
    /// Draw default-ignorable characters as glyphs (`.notdef` if the font lacks them) instead of
    /// hiding them. Set only for marks and invisible characters at the very start of the text,
    /// which have no base character to attach to.
    pub(super) preserve_ignorables: bool,
    pub(super) range: Range<usize>,
}

/// Splits `text` into byte ranges, each drawn with one font.
///
/// A character the `primary` font has (`in_primary`) uses it. Any other character uses the
/// font `fallback` chooses for it, asked once per distinct character within the call.
/// Characters that [attach to the previous one](attaches_to_previous) join its run. At the
/// very start of the text they have nothing to attach to, so they form a primary-font run
/// that keeps them visible (as `.notdef` when the font lacks them) instead of shaping to
/// nothing.
pub(super) fn split_runs(
    text: &str,
    primary: FontKey,
    in_primary: impl Fn(char) -> bool,
    mut fallback: impl FnMut(char) -> FontKey,
) -> Vec<TextRun> {
    let mut chosen: HashMap<char, FontKey> = HashMap::new();
    let mut runs: Vec<TextRun> = Vec::new();
    for (start, c) in text.char_indices() {
        let end = start + c.len_utf8();
        let (key, preserve) = if attaches_to_previous(c) {
            runs.last()
                .map_or((primary, true), |run| (run.font, run.preserve_ignorables))
        } else if in_primary(c) {
            (primary, false)
        } else if let Some(key) = chosen.get(&c) {
            (*key, false)
        } else {
            let key = fallback(c);
            chosen.insert(c, key);
            (key, false)
        };
        match runs.last_mut() {
            Some(last) if last.font == key && last.preserve_ignorables == preserve => {
                last.range.end = end;
            }
            _ => runs.push(TextRun {
                font: key,
                preserve_ignorables: preserve,
                range: start..end,
            }),
        }
    }
    runs
}

/// Whether `c` is a nonspacing or enclosing mark (General Category Mn or Me).
pub(super) fn is_mark(c: char) -> bool {
    let cp = u32::from(c);
    let after = marks::MARKS.partition_point(|&(lo, _)| lo <= cp);
    after > 0 && marks::MARKS.get(after - 1).is_some_and(|&(_, hi)| cp <= hi)
}

/// Characters that stay in the run of the character before them instead of choosing a font of
/// their own: marks (Mn, Me) that are not alphanumeric, emoji modifiers and tags, joiners,
/// invisible format characters and controls. Variation selectors are marks.
///
/// This keeps a base character and its marks in one font, and keeps invisible characters from
/// triggering a fallback search.
pub(super) fn attaches_to_previous(c: char) -> bool {
    if c.is_control() {
        return true;
    }
    if c.is_alphanumeric() {
        return false;
    }
    is_mark(c)
        || matches!(
            u32::from(c),
            0x1F3FB..=0x1F3FF // Emoji skin tone modifiers
                | 0xE0001 | 0xE0020..=0xE007F // Tags
                | 0x00AD // Soft hyphen
                | 0x061C // Arabic letter mark
                | 0x180E // Mongolian vowel separator
                | 0x200B..=0x200F // Zero-width space, non-joiner, joiner, direction marks
                | 0x202A..=0x202E // Bidi embeddings and overrides
                | 0x2060..=0x206F // Word joiner, invisible operators, bidi isolates
                | 0xFEFF // Zero-width no-break space
        )
}

/// The indices of the items of `keys`, grouped by key, groups in order of first appearance.
///
/// One pass with a hash map, so the cost is linear in the number of runs whatever the number of
/// distinct fonts.
pub(super) fn group_by_font(keys: impl Iterator<Item = FontKey>) -> Vec<(FontKey, Vec<usize>)> {
    let mut slots: HashMap<FontKey, usize> = HashMap::new();
    let mut groups: Vec<(FontKey, Vec<usize>)> = Vec::new();
    for (i, key) in keys.enumerate() {
        let slot = *slots.entry(key).or_insert_with(|| {
            groups.push((key, Vec::new()));
            groups.len() - 1
        });
        if let Some((_, members)) = groups.get_mut(slot) {
            members.push(i);
        }
    }
    groups
}
