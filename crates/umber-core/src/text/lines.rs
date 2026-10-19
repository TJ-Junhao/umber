//! The characters drawn text may hold, and where it breaks into lines.
//!
//! Only `"\n"` breaks a line, and only in titles, subtitles and the text of
//! [`AxesMut::text`](crate::AxesMut::text) and [`AxesMut::annotate`](crate::AxesMut::annotate);
//! every other text is one line. No text holds another control character (U+0000 to U+001F,
//! U+007F to U+009F) or a line or paragraph separator (U+2028, U+2029): fonts have no glyphs
//! for them, so they would be drawn as missing-glyph boxes.

use crate::Error;

/// The hint of the error for a line break in text that is one line.
const LINE_BREAK_HINT: &str =
    "line breaks are supported in titles, subtitles, text() and annotate().";

/// Whether a text may break into lines at `"\n"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lines {
    /// One line: `"\n"` is refused like any other control character. The message names what
    /// the text is, in the plural, such as `"axis labels"`.
    One(&'static str),
    /// Lines separated by `"\n"`.
    Many,
}

/// A character a text may not hold, at its index in the text's characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Refused {
    /// The character's index, counted in characters.
    index: usize,
    /// The character.
    character: char,
    /// The lines the text may have.
    lines: Lines,
}

impl Lines {
    /// The first character of `text` that text of these lines may not hold.
    pub(crate) fn refused(self, text: &str) -> Option<Refused> {
        text.chars()
            .enumerate()
            .find(|&(_, c)| match c {
                '\n' => self != Lines::Many,
                '\u{2028}' | '\u{2029}' => true,
                c => c.is_control(),
            })
            .map(|(index, character)| Refused {
                index,
                character,
                lines: self,
            })
    }

    /// Checks that `text`, named `subject` in the message (`"text"`, `"labels[2]"`, `the title
    /// "…"`), holds only characters text of these lines may hold: `Error::Style` under
    /// `context` otherwise.
    pub(crate) fn check_style(
        self,
        context: &'static str,
        subject: &str,
        text: &str,
    ) -> crate::Result<()> {
        match self.refused(text) {
            Some(refused) => {
                Err(Error::style(context, refused.message(subject)).with_hint(refused.hint()))
            }
            None => Ok(()),
        }
    }

    /// As [`Lines::check_style`], with `Error::Data` for names that are data, such as
    /// categories.
    pub(crate) fn check_data(
        self,
        context: &'static str,
        subject: &str,
        text: &str,
    ) -> crate::Result<()> {
        match self.refused(text) {
            Some(refused) => {
                Err(Error::data(context, refused.message(subject)).with_hint(refused.hint()))
            }
            None => Ok(()),
        }
    }
}

impl Refused {
    /// The message for the text named `subject`.
    pub(crate) fn message(&self, subject: &str) -> String {
        let index = self.index;
        match (self.character, self.lines) {
            ('\n', Lines::One(what)) => {
                format!("{subject} holds a line break at index {index}; {what} are one line.")
            }
            (separator @ ('\u{2028}' | '\u{2029}'), _) => format!(
                "{subject} holds {} at index {index}; text cannot hold line or paragraph \
                 separators.",
                name(separator)
            ),
            (control, _) => format!(
                "{subject} holds {} at index {index}; text cannot hold control characters.",
                name(control)
            ),
        }
    }

    /// The hint for the refused character.
    pub(crate) fn hint(&self) -> &'static str {
        match (self.character, self.lines) {
            ('\n', _) => LINE_BREAK_HINT,
            ('\t', Lines::Many) => "use spaces instead of tabs, and \"\\n\" for a line break.",
            ('\t', Lines::One(_)) => "use spaces instead of tabs.",
            ('\r', Lines::Many) => "use \"\\n\" alone for a line break.",
            ('\r', Lines::One(_)) => "remove the carriage return.",
            ('\u{2028}' | '\u{2029}', Lines::Many) => "use \"\\n\" for a line break.",
            ('\u{2028}' | '\u{2029}', Lines::One(_)) => "remove the separator.",
            _ => "remove the character.",
        }
    }
}

/// How messages name the refused character `c`: its code point, and for the ones that look
/// like spacing or a line break, what it is.
fn name(c: char) -> String {
    let what = match c {
        '\t' => " (a tab)",
        '\r' => " (a carriage return)",
        '\u{2028}' => " (a line separator)",
        '\u{2029}' => " (a paragraph separator)",
        _ => "",
    };
    format!("U+{:04X}{what}", u32::from(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    const AXIS: Lines = Lines::One("axis labels");

    #[test]
    fn only_a_line_break_is_allowed_and_only_in_lines() {
        assert_eq!(Lines::Many.refused("Line one\nLine two"), None);
        assert_eq!(Lines::Many.refused("\n\na\n"), None);
        assert_eq!(AXIS.refused("plain text \u{E000} caf\u{e9}"), None);
        let refused = AXIS.refused("caf\u{e9}\nb").unwrap();
        assert_eq!(refused.index, 4);
        for c in [
            '\0', '\t', '\r', '\u{1f}', '\u{7f}', '\u{80}', '\u{85}', '\u{9f}',
        ] {
            for lines in [AXIS, Lines::Many] {
                let refused = lines.refused(&format!("ab{c}")).unwrap();
                assert_eq!((refused.index, refused.character), (2, c));
            }
        }
        for c in ['\u{2028}', '\u{2029}'] {
            assert_eq!(Lines::Many.refused(&format!("{c}")).unwrap().index, 0);
        }
        // Neighbours of the refused ranges are allowed.
        assert_eq!(Lines::Many.refused(" \u{a0}\u{2027}\u{202a}"), None);
    }

    #[test]
    fn messages_name_the_character_and_where_line_breaks_go() {
        let check = |lines: Lines, text: &str| {
            let err = lines.check_style("Axes.title()", "text", text).unwrap_err();
            assert!(matches!(err, Error::Style { .. }), "{err}");
            (err.to_string(), err.hint().unwrap())
        };
        assert_eq!(
            check(AXIS, "Line\none"),
            (
                "Axes.title(): text holds a line break at index 4; axis labels are one line."
                    .to_owned(),
                LINE_BREAK_HINT
            )
        );
        assert_eq!(
            check(Lines::Many, "Tab\there"),
            (
                "Axes.title(): text holds U+0009 (a tab) at index 3; text cannot hold control \
                 characters."
                    .to_owned(),
                "use spaces instead of tabs, and \"\\n\" for a line break."
            )
        );
        assert_eq!(check(AXIS, "a\tb").1, "use spaces instead of tabs.");
        assert_eq!(
            check(Lines::Many, "a\r\nb"),
            (
                "Axes.title(): text holds U+000D (a carriage return) at index 1; text cannot \
                 hold control characters."
                    .to_owned(),
                "use \"\\n\" alone for a line break."
            )
        );
        assert_eq!(check(AXIS, "a\rb").1, "remove the carriage return.");
        assert_eq!(
            check(Lines::Many, "\u{2028}"),
            (
                "Axes.title(): text holds U+2028 (a line separator) at index 0; text cannot \
                 hold line or paragraph separators."
                    .to_owned(),
                "use \"\\n\" for a line break."
            )
        );
        assert_eq!(check(AXIS, "a\u{2029}").1, "remove the separator.");
        assert_eq!(
            check(Lines::Many, "x\u{85}"),
            (
                "Axes.title(): text holds U+0085 at index 1; text cannot hold control characters."
                    .to_owned(),
                "remove the character."
            )
        );
        assert!(check(AXIS, "\0").0.contains("U+0000 at index 0"));
        let data = AXIS
            .check_data("Axes.bar()", "categories[1]", "a\nb")
            .unwrap_err();
        assert!(matches!(data, Error::Data { .. }), "{data}");
        assert!(Lines::Many.check_data("c", "s", "a\nb").is_ok());
    }
}
