//! A label cache keeps all shaping inputs and only unplaced measurements.

use super::*;

#[test]
fn cache_keys_keep_style_bits_text_and_unplaced_layout() {
    let lib = FontLibrary::bundled();
    let cache = LabelCache::new(&lib);
    let text = "a\u{301} ffi 012日";
    let regular = Styles::new(&Theme::pigment(), StyleSizes::default()).tick;
    let styles = [
        regular,
        TextStyle {
            font: crate::Font::Inter,
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
        let expected = measure(&lib, text, style);
        assert_eq!(
            cache.width(text, style).to_bits(),
            expected.width().to_bits()
        );
        assert_eq!(
            cache.width(text, style).to_bits(),
            expected.width().to_bits()
        );
        let mut first = cache.measure(text, style);
        first.x = 123.0;
        first.y = 456.0;
        let again = cache.measure(text, style);
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
        let expected = measure(&lib, text, regular);
        assert_eq!(
            cache.width(text, regular).to_bits(),
            expected.width().to_bits()
        );
        let label = cache.measure(text, regular);
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
    assert!(LabelCache::new(&lib).labels.borrow().is_empty());
}
