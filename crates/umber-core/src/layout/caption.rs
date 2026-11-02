//! The figure caption: a note under everything else in the figure.
//!
//! The caption is the last row of the vertical stack. Its last line ends at the bottom padding;
//! the grid is laid out on the canvas without the caption's [`FigureCaption::room`], so that
//! the panels shrink to make room for it and nothing is drawn over it.

use super::{
    BODY_LINE, HEADER_GAP, PADDING, StyleSizes, Styles, TextBlock, TextRole, baseline_in_line,
    metrics,
};
use crate::error::quoted;
use crate::text::SceneFonts;
use crate::{Figure, Result};

/// The gap above the caption, the theme's spacing between a figure header and what it sits
/// beside ([`HEADER_GAP`], 8 pt). The caption mirrors the header: it is the same kind of
/// figure-level text, at the other end of the stack. The title-to-subtitle gap (2 pt) separates
/// lines of one text and is too tight between a caption and a chart's axis labels.
const CAPTION_GAP: f64 = HEADER_GAP;

/// The figure caption, measured and placed on the canvas.
#[derive(Debug, Clone, Default)]
pub(crate) struct FigureCaption {
    /// The text, placed; `None` when it is empty.
    pub text: Option<TextBlock>,
    /// The height the caption takes: its lines and the gap above them; 0 without a caption.
    room: f64,
}

impl FigureCaption {
    /// Measures the caption of `figure` and places it against the bottom padding, aligned by
    /// the theme's title alignment within the inner width.
    ///
    /// # Errors
    ///
    /// As [`TextBlock::measure`], naming the caption: its text is measured before the metrics
    /// of its style.
    pub fn measure(figure: &Figure, lib: &SceneFonts<'_>) -> Result<FigureCaption> {
        let text = figure.caption_text();
        if text.is_empty() {
            return Ok(FigureCaption::default());
        }
        let theme = figure.theme();
        // The body style is the tick label style without tabular figures, which suit a column
        // of numbers and not prose; the theme gives both the same size.
        let style = Styles::new(
            theme,
            TextRole::Title,
            StyleSizes {
                body: figure.caption_size(),
                ..StyleSizes::default()
            },
        )
        .body;
        let mut block =
            TextBlock::measure(lib, &format!("the caption {}", quoted(text)), text, style)?;
        let (ascent, descent) = metrics(lib, &style)?;
        let line = BODY_LINE * f64::from(style.size);
        let height = line + block.below_first();
        let top = figure.height_in() * 72.0 - PADDING - height;
        let (left, right) = (PADDING, figure.width_in() * 72.0 - PADDING);
        block.place(baseline_in_line(top, line, ascent, descent), |width| {
            theme.title_align().position(width, left, right)
        });
        Ok(FigureCaption {
            text: Some(block),
            room: CAPTION_GAP + height,
        })
    }

    /// The height the layout reserves above the bottom padding: the gap and the lines.
    pub fn room(&self) -> f64 {
        self.room
    }

    /// The height of the lines alone; 0 without a caption.
    pub fn height(&self) -> f64 {
        if self.text.is_some() {
            self.room - CAPTION_GAP
        } else {
            0.0
        }
    }
}
