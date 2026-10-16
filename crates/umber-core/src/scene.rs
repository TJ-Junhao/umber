//! The backend-independent description of a drawn figure.
//!
//! A [`Scene`] is plain data: a list of drawing primitives in points (1/72 inch), with the y
//! axis pointing down and the origin at the top-left corner of the figure. Text enters a scene
//! already shaped and outlined, and keeps its source string and style so that the SVG backend
//! can also write it as editable text. No validation happens when a scene is built; the
//! renderers skip values they cannot draw.
//!
//! # Examples
//!
//! ```
//! use umber_core::scene::{Item, Paint, FillRule, Scene};
//! use umber_core::{Path, Point, Rgba};
//!
//! let mut square = Path::new();
//! square.move_to(Point { x: 0.0, y: 0.0 });
//! square.line_to(Point { x: 10.0, y: 0.0 });
//! square.line_to(Point { x: 10.0, y: 10.0 });
//! square.close();
//!
//! let white = Rgba { r: 255, g: 255, b: 255, a: 255 };
//! let mut scene = Scene::new(100.0, 50.0, white);
//! scene.push(Item::Fill { path: square, paint: Paint::Solid(white), rule: FillRule::NonZero });
//! assert_eq!(scene.leaf_count(), 1);
//! ```

use crate::color::Rgba;
use crate::geometry::{Path, Point};
use crate::text::TextStyle;

/// The deepest group nesting the renderers draw. Content of groups nested deeper than this is
/// omitted, which bounds the renderers' recursion.
///
/// # Examples
///
/// ```
/// assert_eq!(umber_core::scene::MAX_GROUP_DEPTH, 256);
/// ```
pub const MAX_GROUP_DEPTH: usize = 256;

/// A rectangle in points.
///
/// # Examples
///
/// ```
/// use umber_core::scene::Rect;
///
/// let r = Rect { x: 1.0, y: 2.0, width: 30.0, height: 40.0 };
/// assert_eq!(r.x + r.width, 31.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Horizontal extent.
    pub width: f32,
    /// Vertical extent.
    pub height: f32,
}

/// How the ends of an open stroke are drawn.
///
/// # Examples
///
/// ```
/// use umber_core::scene::LineCap;
///
/// assert_ne!(LineCap::Butt, LineCap::Round);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineCap {
    /// The stroke ends flat at the endpoint.
    Butt,
    /// The stroke ends in a semicircle.
    Round,
    /// The stroke extends by half its width, ending flat.
    Square,
}

/// How two stroke segments are joined.
///
/// # Examples
///
/// ```
/// use umber_core::scene::LineJoin;
///
/// assert_ne!(LineJoin::Miter, LineJoin::Bevel);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineJoin {
    /// Sharp corner.
    Miter,
    /// Rounded corner.
    Round,
    /// Flattened corner.
    Bevel,
}

/// Which points a fill considers inside a path.
///
/// # Examples
///
/// ```
/// use umber_core::scene::FillRule;
///
/// assert_ne!(FillRule::NonZero, FillRule::EvenOdd);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FillRule {
    /// Non-zero winding number.
    NonZero,
    /// Odd crossing count.
    EvenOdd,
}

/// How a stroke is drawn. `dash` is in points, alternating on and off; empty means solid.
///
/// # Examples
///
/// ```
/// use umber_core::scene::{LineCap, LineJoin, StrokeStyle};
/// use umber_core::Rgba;
///
/// let style = StrokeStyle {
///     color: Rgba { r: 0, g: 0, b: 0, a: 255 },
///     width: 1.5,
///     dash: vec![4.0, 2.0],
///     cap: LineCap::Butt,
///     join: LineJoin::Miter,
/// };
/// assert_eq!(style.dash.len(), 2);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct StrokeStyle {
    /// Stroke color.
    pub color: Rgba,
    /// Stroke width in points.
    pub width: f32,
    /// Alternating dash and gap lengths in points.
    pub dash: Vec<f32>,
    /// End cap.
    pub cap: LineCap,
    /// Corner join.
    pub join: LineJoin,
}

/// One color stop of a gradient.
///
/// # Examples
///
/// ```
/// use umber_core::scene::GradientStop;
/// use umber_core::Rgba;
///
/// let stop = GradientStop { offset: 0.5, color: Rgba { r: 9, g: 9, b: 9, a: 255 } };
/// assert_eq!(stop.offset, 0.5);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct GradientStop {
    /// Position along the gradient, in `0..=1`.
    pub offset: f32,
    /// Color at that position.
    pub color: Rgba,
}

/// A linear gradient between two points, in scene coordinates.
///
/// # Examples
///
/// ```
/// use umber_core::scene::{GradientStop, LinearGradient};
/// use umber_core::{Point, Rgba};
///
/// let black = Rgba { r: 0, g: 0, b: 0, a: 255 };
/// let white = Rgba { r: 255, g: 255, b: 255, a: 255 };
/// let gradient = LinearGradient {
///     start: Point { x: 0.0, y: 0.0 },
///     end: Point { x: 0.0, y: 100.0 },
///     stops: vec![GradientStop { offset: 0.0, color: black }, GradientStop { offset: 1.0, color: white }],
/// };
/// assert_eq!(gradient.stops.len(), 2);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct LinearGradient {
    /// Where the gradient starts.
    pub start: Point,
    /// Where the gradient ends.
    pub end: Point,
    /// Color stops, in increasing offset order.
    pub stops: Vec<GradientStop>,
}

/// What a fill is painted with.
///
/// # Examples
///
/// ```
/// use umber_core::scene::Paint;
/// use umber_core::Rgba;
///
/// let paint = Paint::Solid(Rgba { r: 1, g: 2, b: 3, a: 255 });
/// assert!(matches!(paint, Paint::Solid(_)));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Paint {
    /// A single color.
    Solid(Rgba),
    /// A linear gradient.
    Linear(LinearGradient),
    /// A sampled color gradient. Opaque neighboring stops mix in Oklab in PNG; dense
    /// samples also describe the same curve to SVG's linear gradient renderer.
    LinearGradient {
        /// The point at offset zero.
        from: Point,
        /// The point at offset one.
        to: Point,
        /// Positions and colors, ordered along the gradient.
        stops: Vec<(f32, Rgba)>,
    },
}

/// Converts sampled stops to the shared gradient representation for normalization.
pub(crate) fn sampled_gradient(from: Point, to: Point, stops: &[(f32, Rgba)]) -> LinearGradient {
    LinearGradient {
        start: from,
        end: to,
        stops: stops
            .iter()
            .map(|&(offset, color)| GradientStop { offset, color })
            .collect(),
    }
}

/// Text that has already been shaped and outlined, plus what is needed to re-emit it as
/// editable SVG text.
///
/// # Examples
///
/// ```
/// use umber_core::scene::{Item, Scene};
/// use umber_core::{Font, FontLibrary, Point, Rgba, TextStyle, Weight};
///
/// let white = Rgba { r: 255, g: 255, b: 255, a: 255 };
/// let style = TextStyle { font: Font::Inter, weight: Weight::Regular, size: 12.0, tabular_figures: false };
/// let mut scene = Scene::new(100.0, 40.0, white);
/// scene.push_text(FontLibrary::global(), "Hi", style, Point { x: 5.0, y: 20.0 }, white);
/// let Some(Item::Text(item)) = scene.items.first() else { unreachable!() };
/// assert_eq!(item.text, "Hi");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TextItem {
    /// The source string.
    pub text: String,
    /// The style the text was laid out with.
    pub style: TextStyle,
    /// Left end of the baseline.
    pub origin: Point,
    /// Text color.
    pub color: Rgba,
    /// Glyph outlines, already placed at `origin`.
    pub outline: Path,
    /// Advance width in points.
    pub width: f32,
}

/// One drawing primitive.
///
/// # Examples
///
/// ```
/// use umber_core::scene::Item;
///
/// let group = Item::Group { clip: None, items: Vec::new() };
/// assert!(matches!(group, Item::Group { .. }));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// A filled path.
    Fill {
        /// The outline to fill.
        path: Path,
        /// What to fill with.
        paint: Paint,
        /// Which points count as inside.
        rule: FillRule,
    },
    /// A stroked path.
    Stroke {
        /// The path to stroke.
        path: Path,
        /// How to stroke it.
        style: StrokeStyle,
    },
    /// One marker shape (centered at (0, 0)) drawn at many positions.
    Markers {
        /// The marker shape.
        shape: Path,
        /// Fill color, if filled.
        fill: Option<Rgba>,
        /// Stroke style, if stroked.
        stroke: Option<StrokeStyle>,
        /// Where each copy of the shape is centered.
        positions: Vec<Point>,
    },
    /// Shaped text.
    Text(TextItem),
    /// Items drawn in order, optionally clipped to a rectangle.
    Group {
        /// The clip rectangle, if any.
        clip: Option<Rect>,
        /// The items in the group.
        items: Vec<Item>,
    },
}

/// A figure as a list of drawing primitives.
///
/// # Examples
///
/// ```
/// use umber_core::scene::Scene;
/// use umber_core::Rgba;
///
/// let scene = Scene::new(640.0, 400.0, Rgba { r: 255, g: 255, b: 255, a: 255 });
/// assert_eq!(scene.leaf_count(), 0);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// Width in points.
    pub width: f32,
    /// Height in points.
    pub height: f32,
    /// Background color, drawn under every item.
    pub background: Rgba,
    /// Top-level items, drawn in order.
    pub items: Vec<Item>,
}

impl Drop for Scene {
    /// Drops nested groups without recursion, so a deep chain cannot overflow the stack.
    fn drop(&mut self) {
        let mut stack = std::mem::take(&mut self.items);
        while let Some(item) = stack.pop() {
            if let Item::Group { items, .. } = item {
                stack.extend(items);
            }
        }
    }
}

impl Scene {
    /// Creates an empty scene.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::scene::Scene;
    /// use umber_core::Rgba;
    ///
    /// let scene = Scene::new(10.0, 20.0, Rgba { r: 0, g: 0, b: 0, a: 0 });
    /// assert_eq!((scene.width, scene.height), (10.0, 20.0));
    /// ```
    #[must_use]
    pub fn new(width: f32, height: f32, background: Rgba) -> Scene {
        Scene {
            width,
            height,
            background,
            items: Vec::new(),
        }
    }

    /// Appends an item.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::scene::{Item, Scene};
    /// use umber_core::Rgba;
    ///
    /// let mut scene = Scene::new(10.0, 10.0, Rgba { r: 0, g: 0, b: 0, a: 0 });
    /// scene.push(Item::Group { clip: None, items: Vec::new() });
    /// assert_eq!(scene.items.len(), 1);
    /// ```
    pub fn push(&mut self, item: Item) {
        self.items.push(item);
    }

    /// Number of leaf items (fills, strokes, markers and text), counted through groups.
    ///
    /// # Examples
    ///
    /// ```
    /// use umber_core::scene::{Item, Scene};
    /// use umber_core::Rgba;
    ///
    /// let mut scene = Scene::new(10.0, 10.0, Rgba { r: 0, g: 0, b: 0, a: 0 });
    /// scene.push(Item::Group { clip: None, items: Vec::new() });
    /// assert_eq!(scene.leaf_count(), 0);
    /// ```
    #[must_use]
    pub fn leaf_count(&self) -> usize {
        fn count(items: &[Item]) -> usize {
            items
                .iter()
                .map(|item| match item {
                    Item::Group { items, .. } => count(items),
                    _ => 1,
                })
                .sum()
        }
        count(&self.items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLACK: Rgba = Rgba {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    fn leaf() -> Item {
        Item::Stroke {
            path: Path::new(),
            style: StrokeStyle {
                color: BLACK,
                width: 1.0,
                dash: Vec::new(),
                cap: LineCap::Butt,
                join: LineJoin::Miter,
            },
        }
    }

    #[test]
    fn leaf_count_counts_through_groups() {
        let mut scene = Scene::new(10.0, 10.0, BLACK);
        let inner = Item::Group {
            clip: None,
            items: vec![leaf(), leaf()],
        };
        scene.push(Item::Group {
            clip: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 5.0,
                height: 5.0,
            }),
            items: vec![inner],
        });
        scene.push(leaf());
        assert_eq!(scene.leaf_count(), 3);
    }
}
