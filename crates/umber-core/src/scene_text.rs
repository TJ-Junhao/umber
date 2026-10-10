//! Turning shaped text into scene records.
//!
//! [`TextItem::from_layout`] is the one place that fills a [`TextItem`] from a [`TextLayout`];
//! [`Scene::push_text`] shapes with a [`FontLibrary`] and then uses it. Keeping both here leaves
//! [`crate::scene`] as plain records with no dependency on the text shaping service.

use crate::color::Rgba;
use crate::geometry::Point;
use crate::scene::{Item, Scene, TextItem};
use crate::text::{FontLibrary, TextLayout, TextStyle};

impl TextItem {
    /// The record for `text`, already shaped as `layout` with `style`, drawn at `origin`.
    ///
    /// The outline is built from `layout` placed at `origin` and is owned by the record, so it
    /// does not borrow `layout` or the library that shaped it.
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
        }
    }
}

impl Scene {
    /// Lays out `text` with `library`, outlines it at `origin`, and pushes a [`TextItem`].
    ///
    /// `origin` is the left end of the baseline. Empty text still pushes an item, with an empty
    /// outline and zero width.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::scene::Scene;
    /// use umber_core::{Font, FontLibrary, Point, Rgba, TextStyle, Weight};
    ///
    /// let black = Rgba { r: 0, g: 0, b: 0, a: 255 };
    /// let style = TextStyle { font: Font::Geist, weight: Weight::Regular, size: 10.0, tabular_figures: false };
    /// let mut scene = Scene::new(100.0, 30.0, black);
    /// scene.push_text(FontLibrary::global(), "Axis", style, Point { x: 2.0, y: 15.0 }, black);
    /// assert_eq!(scene.leaf_count(), 1);
    /// ```
    pub fn push_text(
        &mut self,
        library: &FontLibrary,
        text: &str,
        style: TextStyle,
        origin: Point,
        color: Rgba,
    ) {
        let layout = library.layout(text, &style);
        self.items.push(Item::Text(TextItem::from_layout(
            text.to_owned(),
            style,
            origin,
            color,
            &layout,
        )));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::{Font, Weight};

    const BLACK: Rgba = Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    fn style() -> TextStyle {
        TextStyle {
            font: Font::Inter,
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
        scene.push_text(FontLibrary::global(), "Hamburg", style(), origin, BLACK);
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
        scene.push_text(FontLibrary::global(), "Revenue", s, Point::default(), BLACK);
        let item = text_item(&scene);
        assert_eq!(item.text, "Revenue");
        assert_eq!(item.style, s);
        assert_eq!(
            item.width,
            FontLibrary::global().layout("Revenue", &s).width()
        );
    }

    #[test]
    fn empty_text_pushes_an_empty_item() {
        let mut scene = Scene::new(10.0, 10.0, BLACK);
        scene.push_text(FontLibrary::global(), "", style(), Point::default(), BLACK);
        assert_eq!(scene.leaf_count(), 1);
        let item = text_item(&scene);
        assert_eq!(item.width, 0.0);
        assert!(item.outline.is_empty());
    }

    #[test]
    fn from_layout_fills_every_field_and_outlives_the_layout() {
        let s = style();
        let origin = Point { x: 12.0, y: 30.0 };
        let color = Rgba {
            r: 10,
            g: 20,
            b: 30,
            a: 200,
        };
        let item = {
            let layout = FontLibrary::global().layout("Signal", &s);
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
        scene.push_text(FontLibrary::global(), "Signal", s, origin, color);
        assert_eq!(text_item(&scene), &item);
    }
}
