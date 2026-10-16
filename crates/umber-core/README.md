# umber-core

`umber-core` is the rendering core of [umber](https://github.com/TJ-Junhao/umber), a plotting library for Python. It draws line, step, scatter, area, stacked area, bar, histogram, box, violin, bubble, error bar, band, stem, reference-line, heatmap, mesh, 2-D histogram and hexbin charts, including grouped and stacked bars, grouped boxes and violins, reference spans, horizontal bars, boxes and violins, and color-mapped points. Axes are linear, log, category or time axes, and color-mapped marks come with colormaps and colorbars. Figures can have a single axes or a grid of panels with shared axes, spanning panels, figure titles and merged or per-panel legends. It renders to PNG and SVG, has no Python dependency and can be used directly from Rust.

```rust
use umber_core::{AxisData, Figure, LineOptions, ScaleKind, Theme};

let mut figure = Figure::new(6.0, 4.0, Theme::pigment())?;
figure
    .add_axes(ScaleKind::Linear, ScaleKind::Linear)?
    .title("Signal")
    .line(AxisData::Numbers(vec![0.0, 1.0, 2.0, 3.0]), &[1.0, 3.0, 2.0, 4.0], LineOptions::default())?;
std::fs::write("signal.png", figure.to_png(144)?)?;
```

The API is `0.x` and not yet stable; it may change or be removed without a deprecation period.

## License

Licensed under either of MIT or Apache-2.0, at your option (`MIT OR Apache-2.0`).

## Bundled fonts

This crate bundles fonts under the SIL Open Font License 1.1. The license texts are in `fonts/LICENSES` and the provenance of each font is in `fonts/MANIFEST.md`.
