# umber-core

`umber-core` is the rendering core of [umber](https://github.com/TJ-Junhao/umber), a plotting library for Python. It lays out figures, axes and marks (lines, scatter points, areas, bars and histograms) and renders them to PNG and SVG. It has no Python dependency and can be used directly from Rust.

```rust
use umber_core::{AxisData, Figure, LineOptions, ScaleKind, Theme};

let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
figure
    .add_axes(ScaleKind::Linear, ScaleKind::Linear)?
    .title("Signal")
    .line(AxisData::Numbers(vec![0.0, 1.0, 2.0, 3.0]), &[1.0, 3.0, 2.0, 4.0], LineOptions::default())?;
std::fs::write("signal.png", figure.to_png(144)?)?;
```

The API is `0.x` and may change between minor versions; patch releases within a minor version are compatible.

## License

Licensed under either of MIT or Apache-2.0, at your option (`MIT OR Apache-2.0`).

## Bundled fonts

This crate bundles fonts under the SIL Open Font License 1.1. The license texts are in `fonts/LICENSES` and the provenance of each font is in `fonts/MANIFEST.md`.
