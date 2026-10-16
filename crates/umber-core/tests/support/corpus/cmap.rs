//! Output cases for the built-in colormaps, custom colormaps and the theme defaults.

use std::fmt::Write as _;

use umber_core::{Cmap, Rgba};

use super::super::{Output, record, themes};

/// The positions a colormap is sampled at.
const SAMPLES: [f64; 7] = [-1.0, 0.0, 0.25, 0.5, 0.75, 1.0, 2.0];

/// One line per sample: the position and the color.
fn sampled(out: &mut String, label: &str, cmap: &Cmap) {
    let _ = writeln!(out, "{label} stops {}", cmap.colors().len());
    for t in SAMPLES {
        let _ = writeln!(
            out,
            "{label} at {} {}",
            record::f64(t),
            record::rgba(cmap.at(t))
        );
    }
}

/// Emits the colormaps and the theme defaults.
pub fn cases(sink: &mut impl FnMut(Output)) {
    let mut defaults = String::new();
    for (name, theme) in themes()
        .into_iter()
        .chain([("default", umber_core::Theme::default())])
    {
        sampled(&mut defaults, &format!("{name}: cmap"), theme.cmap());
        sampled(
            &mut defaults,
            &format!("{name}: diverging_cmap"),
            theme.diverging_cmap(),
        );
    }
    sink(Output {
        kind: "api",
        file: "themes-cmap.txt".into(),
        bytes: defaults.into_bytes(),
    });

    let mut out = String::new();
    for (name, cmap) in [
        ("viridis", Cmap::viridis()),
        ("magma", Cmap::magma()),
        ("cividis", Cmap::cividis()),
        ("blue_red", Cmap::blue_red()),
    ] {
        sampled(&mut out, name, &cmap);
        sampled(&mut out, &format!("{name} reversed"), &cmap.reversed());
    }
    let custom = [Rgba::BLACK, Rgba::WHITE];
    sampled(
        &mut out,
        "black-white",
        &Cmap::from_colors(&custom).unwrap(),
    );
    let translucent = [Rgba::opaque(200, 40, 20), Rgba::hex("#2040C080").unwrap()];
    let _ = match Cmap::from_colors(&translucent) {
        Ok(cmap) => writeln!(out, "translucent: ok stops {}", cmap.colors().len()),
        Err(e) => writeln!(out, "translucent: {}", record::error(&e)),
    };
    for count in [0, 1, 2, 16, 17] {
        let result = Cmap::from_colors(&vec![Rgba::RED; count]);
        let _ = match result {
            Ok(cmap) => writeln!(out, "from_colors {count}: ok stops {}", cmap.colors().len()),
            Err(e) => writeln!(out, "from_colors {count}: {}", record::error(&e)),
        };
    }
    sink(Output {
        kind: "api",
        file: "cmaps.txt".into(),
        bytes: out.into_bytes(),
    });
}
