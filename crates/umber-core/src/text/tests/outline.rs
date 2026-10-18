//! Outlines of laid-out text, including layouts that outlive their library.

use super::fixtures::{LayoutBundled, omega_lib, style};
use crate::fonts::BundledFont;
use crate::geometry::Point;
use crate::text::FontLibrary;

#[test]
fn outline_sits_on_the_baseline_within_the_advance() {
    let lib = FontLibrary::with_fallbacks(vec![]);
    let layout = lib.layout("Hg", &style(BundledFont::Inter, 12.0));
    let path = layout.outline(Point { x: 5.0, y: 50.0 });
    let (min, max) = path.bounds().expect("non-empty");
    assert!(min.x >= 5.0 - 1.0 && max.x <= 5.0 + layout.width() + 1.0);
    assert!(
        min.y < 50.0 && max.y > 50.0,
        "H above and g below the baseline"
    );
    assert!(min.y >= 50.0 - layout.ascent() - 1.0 && max.y <= 50.0 + layout.descent() + 1.0);
}

#[test]
fn layout_outlines_after_its_library_is_dropped() {
    let text = format!("A{}", crate::fonts::FALLBACK_TEST_CODEPOINT);
    let lib = omega_lib();
    let layout = lib.layout(&text, &style(BundledFont::Inter, 10.0));
    assert_eq!(layout.runs().len(), 2);
    let before = layout.outline(Point { x: 3.0, y: 7.0 });
    drop(lib);
    let after = layout.outline(Point { x: 3.0, y: 7.0 });
    assert_eq!(before, after);
    let a_only = FontLibrary::with_fallbacks(vec![]).layout("A", &style(BundledFont::Inter, 10.0));
    assert!(after.bounds().expect("outline").1.x > 3.0 + a_only.width());
    // Clones share the font handles.
    assert_eq!(layout.clone().outline(Point { x: 3.0, y: 7.0 }), before);
}
