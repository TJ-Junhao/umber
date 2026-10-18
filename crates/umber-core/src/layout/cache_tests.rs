//! A label cache keeps all shaping inputs and only unplaced measurements.

use super::*;

#[test]
fn cache_keys_keep_style_bits_text_and_unplaced_layout() {
    let lib = FontLibrary::bundled();
    let fonts = figure_fonts(&lib, &Theme::pigment()).unwrap();
    let cache = LabelCache::new(&fonts);
    let text = "a\u{301} ffi 012日";
    let regular = Styles::new(&Theme::pigment(), TextRole::Title, StyleSizes::default()).tick;
    let styles = [
        regular,
        TextStyle {
            face: TITLE_FACE,
            ..regular
        },
        TextStyle {
            weight: Weight::SemiBold,
            ..regular
        },
        TextStyle {
            size: f32::from_bits(regular.size.to_bits() + 1),
            ..regular
        },
        TextStyle {
            tabular_figures: false,
            ..regular
        },
    ];
    for style in styles {
        let expected = measure(&fonts, text, style).unwrap();
        assert_eq!(
            cache.width(text, style).unwrap().to_bits(),
            expected.width().to_bits()
        );
        assert_eq!(
            cache.width(text, style).unwrap().to_bits(),
            expected.width().to_bits()
        );
        let mut first = cache.measure(text, style).unwrap();
        first.x = 123.0;
        first.y = 456.0;
        let again = cache.measure(text, style).unwrap();
        assert_eq!(again.text, expected.text);
        assert_eq!(again.style, expected.style);
        assert_eq!(
            format!("{:?}", again.layout),
            format!("{:?}", expected.layout)
        );
        assert_eq!(
            (again.x.to_bits(), again.y.to_bits()),
            (0.0_f64.to_bits(), 0.0_f64.to_bits())
        );
    }
    assert_eq!(cache.labels.borrow().len(), styles.len());
    assert!(
        cache
            .labels
            .borrow()
            .values()
            .all(|labels| labels.len() == 1)
    );
    let long = "W".repeat(5000);
    for text in ["", "ffi", &long] {
        let expected = measure(&fonts, text, regular).unwrap();
        assert_eq!(
            cache.width(text, regular).unwrap().to_bits(),
            expected.width().to_bits()
        );
        let label = cache.measure(text, regular).unwrap();
        assert_eq!(label.text, expected.text);
        assert_eq!(
            format!("{:?}", label.layout),
            format!("{:?}", expected.layout)
        );
    }
    assert_eq!(
        cache
            .labels
            .borrow()
            .values()
            .map(HashMap::len)
            .sum::<usize>(),
        styles.len() + 3
    );
    assert!(LabelCache::new(&fonts).labels.borrow().is_empty());
}
