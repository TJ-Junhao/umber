//! Turning shaped text into scene records.
//!
//! [`TextItem::from_layout`] is the one place that fills a [`TextItem`] from a [`TextLayout`];
//! [`Scene::push_text`] shapes with the [`SceneFonts`] of a scene and then uses it. Keeping both
//! here leaves [`crate::scene`] as plain records with no dependency on the text shaping service.

use crate::Result;
use crate::color::Rgba;
use crate::fonts::Font;
use crate::geometry::Point;
use crate::scene::{Item, Scene, TextItem};
use crate::text::{FaceId, FontCapture, SceneFonts, TextLayout, TextStyle};

impl TextItem {
    /// The record for `text`, already shaped as `layout` with `style`, drawn at `origin`.
    ///
    /// The outline is built from `layout` placed at `origin`, and the glyphs are those of
    /// `layout`. Both are owned by the record, so it does not borrow `layout` or the library
    /// that shaped it.
    pub(crate) fn from_layout(
        text: String,
        style: TextStyle,
        origin: Point,
        color: Rgba,
        layout: &TextLayout,
    ) -> TextItem {
        TextItem {
            text,
            style,
            origin,
            color,
            outline: layout.outline(origin),
            width: layout.width(),
            glyphs: layout.glyphs(),
        }
    }
}

impl Scene {
    /// Lays out `text` with `fonts`, outlines it at `origin`, and pushes a [`TextItem`].
    ///
    /// `origin` is the left end of the baseline. `style.face` names a font of `fonts`; the
    /// item names the same font by its index in this scene's [`fonts`](Scene::fonts), which
    /// gains the font when it does not hold it yet. Empty text still pushes an item, with an
    /// empty outline and zero width. A system font the text falls back to is copied into the
    /// item's [`glyphs`](TextItem::glyphs), sharing the copy other text items of the scene
    /// already hold, so later changes to the font file do not change the scene.
    ///
    /// # Errors
    ///
    /// As [`SceneFonts::layout`]; then the scene is unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::scene::Scene;
    /// use umber_core::{FaceId, Font, FontLibrary, Point, Rgba, TextStyle, Weight};
    ///
    /// let black = Rgba { r: 0, g: 0, b: 0, a: 255 };
    /// let fonts = FontLibrary::global().with_figure_fonts(&[Font::INTER, Font::GEIST])?;
    /// let style = TextStyle { face: FaceId::new(1), weight: Weight::Regular, size: 10.0, tabular_figures: false };
    /// let mut scene = Scene::new(100.0, 30.0, black);
    /// scene.push_text(&fonts, "Axis", style, Point { x: 2.0, y: 15.0 }, black)?;
    /// assert_eq!(scene.leaf_count(), 1);
    /// assert_eq!(scene.fonts, [Font::GEIST]);
    /// # Ok::<(), umber_core::Error>(())
    /// ```
    pub fn push_text(
        &mut self,
        fonts: &SceneFonts<'_>,
        text: &str,
        style: TextStyle,
        origin: Point,
        color: Rgba,
    ) -> Result<()> {
        let checked = fonts.check(text, &style)?;
        let layout = fonts.layout_checked(text, &checked);
        let style = TextStyle {
            face: self.face_of(checked.font()),
            ..style
        };
        let mut item = TextItem::from_layout(text.to_owned(), style, origin, color, &layout);
        let mut capture = FontCapture::default();
        for_each_text(&self.items, &mut |known| capture.learn(&known.glyphs));
        capture.capture(&mut item.glyphs);
        self.items.push(Item::Text(item));
        Ok(())
    }

    /// Copies the system fonts the scene's text was shaped with into its text items, one copy
    /// per font program shared by every item that uses it (see [`Scene::push_text`]), so the
    /// scene no longer depends on the font files.
    pub(crate) fn capture_fonts(&mut self) {
        let mut capture = FontCapture::default();
        let mut stack: Vec<&mut Item> = self.items.iter_mut().collect();
        while let Some(item) = stack.pop() {
            match item {
                Item::Text(text) => capture.capture(&mut text.glyphs),
                Item::Group { items, .. } => stack.extend(items.iter_mut()),
                Item::Fill { .. }
                | Item::Stroke { .. }
                | Item::Markers { .. }
                | Item::Image { .. } => {}
            }
        }
    }

    /// The face of `font` in this scene, adding the font to [`fonts`](Scene::fonts) when the
    /// scene does not hold it yet.
    fn face_of(&mut self, font: &Font) -> FaceId {
        let index = match self.fonts.iter().position(|known| known == font) {
            Some(index) => index,
            None => {
                self.fonts.push(font.clone());
                self.fonts.len() - 1
            }
        };
        FaceId::new(index)
    }
}

/// Calls `f` on every text item of `items`, through groups.
fn for_each_text(items: &[Item], f: &mut impl FnMut(&TextItem)) {
    let mut stack: Vec<&Item> = items.iter().collect();
    while let Some(item) = stack.pop() {
        match item {
            Item::Text(text) => f(text),
            Item::Group { items, .. } => stack.extend(items.iter()),
            Item::Fill { .. } | Item::Stroke { .. } | Item::Markers { .. } | Item::Image { .. } => {
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::{Font, Weight};
    use crate::text::FontLibrary;

    const BLACK: Rgba = Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    /// Geist as face 0 and Inter as face 1.
    fn fonts() -> SceneFonts<'static> {
        FontLibrary::global()
            .with_figure_fonts(&[Font::GEIST, Font::INTER])
            .unwrap()
    }

    /// Inter, face 1 of [`fonts`].
    fn style() -> TextStyle {
        TextStyle {
            face: FaceId::new(1),
            weight: Weight::Regular,
            size: 12.0,
            tabular_figures: false,
        }
    }

    fn text_item(scene: &Scene) -> &TextItem {
        match scene.items.first() {
            Some(Item::Text(item)) => item,
            other => panic!("expected a text item, got {other:?}"),
        }
    }

    #[test]
    fn push_text_places_the_outline_at_the_origin() {
        let mut scene = Scene::new(200.0, 100.0, BLACK);
        let origin = Point { x: 40.0, y: 60.0 };
        scene
            .push_text(&fonts(), "Hamburg", style(), origin, BLACK)
            .unwrap();
        let item = text_item(&scene);
        let (min, max) = item.outline.bounds().expect("outline has geometry");
        assert!(min.x >= origin.x - 1.0, "min.x {}", min.x);
        assert!(max.x <= origin.x + item.width + 1.0, "max.x {}", max.x);
        assert_eq!(item.origin, origin);
    }

    #[test]
    fn push_text_records_source_and_width() {
        let mut scene = Scene::new(200.0, 100.0, BLACK);
        let s = style();
        scene
            .push_text(&fonts(), "Revenue", s, Point::default(), BLACK)
            .unwrap();
        let item = text_item(&scene);
        assert_eq!(item.text, "Revenue");
        assert_eq!(item.width, fonts().layout("Revenue", &s).unwrap().width());
    }

    #[test]
    fn pushed_text_names_its_font_in_the_scene() {
        let mut scene = Scene::new(200.0, 100.0, BLACK);
        let fonts = fonts();
        let geist = TextStyle {
            face: FaceId::new(0),
            ..style()
        };
        scene
            .push_text(&fonts, "Inter", style(), Point::default(), BLACK)
            .unwrap();
        scene
            .push_text(&fonts, "Inter again", style(), Point::default(), BLACK)
            .unwrap();
        scene
            .push_text(&fonts, "Geist", geist, Point::default(), BLACK)
            .unwrap();
        assert_eq!(scene.fonts, [Font::INTER, Font::GEIST]);
        let faces: Vec<FaceId> = scene
            .items
            .iter()
            .map(|item| match item {
                Item::Text(text) => text.style.face,
                other => panic!("expected a text item, got {other:?}"),
            })
            .collect();
        assert_eq!(faces, [FaceId::new(0), FaceId::new(0), FaceId::new(1)]);
        assert_eq!(scene.font(FaceId::new(1)), Some(&Font::GEIST));
        assert_eq!(scene.font(FaceId::new(2)), None);
        let item = text_item(&scene);
        assert_eq!(
            item.style,
            TextStyle {
                face: FaceId::new(0),
                ..style()
            }
        );
    }

    #[test]
    fn text_in_a_face_the_fonts_lack_is_an_error_and_pushes_nothing() {
        let mut scene = Scene::new(200.0, 100.0, BLACK);
        let missing = TextStyle {
            face: FaceId::new(2),
            ..style()
        };
        let err = scene
            .push_text(&fonts(), "Lost", missing, Point::default(), BLACK)
            .unwrap_err();
        assert!(matches!(err, crate::Error::Usage { .. }), "{err}");
        assert_eq!(
            err.to_string(),
            "SceneFonts.layout(): the text style names face 2, but the last face is 1; a face \
             must be the index of one of the fonts."
        );
        assert!(scene.items.is_empty());
        assert!(scene.fonts.is_empty());
    }

    #[test]
    fn empty_text_pushes_an_empty_item() {
        let mut scene = Scene::new(10.0, 10.0, BLACK);
        scene
            .push_text(&fonts(), "", style(), Point::default(), BLACK)
            .unwrap();
        assert_eq!(scene.leaf_count(), 1);
        let item = text_item(&scene);
        assert_eq!(item.width, 0.0);
        assert!(item.outline.is_empty());
    }

    #[test]
    fn from_layout_fills_every_field_and_outlives_the_layout() {
        let s = TextStyle {
            face: FaceId::new(0),
            ..style()
        };
        let fonts = FontLibrary::global()
            .with_figure_fonts(&[Font::INTER])
            .unwrap();
        let origin = Point { x: 12.0, y: 30.0 };
        let color = Rgba {
            r: 10,
            g: 20,
            b: 30,
            a: 200,
        };
        let item = {
            let layout = fonts.layout("Signal", &s).unwrap();
            let item = TextItem::from_layout("Signal".to_owned(), s, origin, color, &layout);
            assert_eq!(item.outline, layout.outline(origin));
            assert_eq!(item.width, layout.width());
            item
        };
        assert_eq!(item.text, "Signal");
        assert_eq!(item.style, s);
        assert_eq!(item.origin, origin);
        assert_eq!(item.color, color);
        assert!(!item.outline.is_empty());

        let mut scene = Scene::new(100.0, 50.0, BLACK);
        scene.push_text(&fonts, "Signal", s, origin, color).unwrap();
        assert_eq!(text_item(&scene), &item);
    }

    #[test]
    fn pushed_text_keeps_the_glyphs_its_outline_was_drawn_from() {
        let fonts = fonts();
        let mut scene = Scene::new(200.0, 100.0, BLACK);
        scene
            .push_text(&fonts, "Revenue", style(), Point { x: 3.0, y: 40.0 }, BLACK)
            .unwrap();
        let item = text_item(&scene);
        let layout = fonts.layout("Revenue", &style()).unwrap();
        assert_eq!(item.glyphs, layout.glyphs());
        assert_eq!(item.glyphs.glyph_count(), 7);
        assert_eq!(item.glyphs.size(), 12.0);
        // The glyphs are placed relative to the origin, as the layout placed them.
        let run = &item.glyphs.runs()[0];
        assert_eq!(run.glyphs()[0].x, 0.0);
        assert!(run.glyphs().windows(2).all(|pair| pair[0].x < pair[1].x));
    }

    #[test]
    fn glyph_texts_spell_the_source_text() {
        let fonts = fonts();
        for text in ["Revenue 2024", "office ffi", "naïve café", "a\u{301}b"] {
            let glyphs = fonts.layout(text, &style()).unwrap().glyphs();
            let spelled: String = glyphs
                .runs()
                .iter()
                .flat_map(|run| run.glyph_texts())
                .collect();
            assert_eq!(spelled, text);
            for run in glyphs.runs() {
                assert_eq!(run.glyph_texts().len(), run.glyphs().len());
            }
        }
    }

    #[test]
    fn runs_of_each_font_name_their_font() {
        let library = FontLibrary::with_fallbacks(vec![
            include_bytes!("../tests/fixtures/fallback-only.ttf").to_vec(),
        ]);
        let fonts = library.with_figure_fonts(&[Font::INTER]).unwrap();
        let s = TextStyle {
            face: FaceId::new(0),
            ..style()
        };
        let text = format!("A{}B", crate::fonts::FALLBACK_TEST_CODEPOINT);
        let glyphs = fonts.layout(&text, &s).unwrap().glyphs();
        let runs = glyphs.runs();
        assert_eq!(runs.len(), 3);
        let program = |i: usize| runs[i].run_font().program().unwrap();
        assert_eq!(program(0), program(2));
        assert_ne!(program(0), program(1));
        assert_eq!(
            runs[1].glyph_texts(),
            [crate::fonts::FALLBACK_TEST_CODEPOINT.to_string()]
        );
        // The fallback font's bytes can be read again for embedding.
        let read = runs[1]
            .run_font()
            .with_data(|data, index| Some((data.len(), index)));
        assert_eq!(read.map(|(_, index)| index), Some(0));
    }

    #[test]
    fn hand_built_text_has_no_glyphs() {
        assert!(crate::text::TextGlyphs::default().is_empty());
        assert_eq!(crate::text::TextGlyphs::default().glyph_count(), 0);
    }
}
