//! The integer values that stand for the core enums on the Python side.
//!
//! Each public Python enum is a plain `enum.Enum` whose member values are the integers listed
//! here. One table per enum generates both directions of the mapping, so a variant added to a
//! core enum fails to compile until its table lists it, and a duplicated value or variant is an
//! unreachable pattern.

use pyo3::prelude::*;
use pyo3::types::PyDict;
use umber_core::{
    Align, ColorbarPosition, Dash, FillStyle, Font, GridLines, LegendMode, LegendPosition,
    LegendStyle, Marker, Render, Share, Step, SvgText,
};

use crate::errors::internal_value;

/// A core enum with a fixed integer value per variant.
pub(crate) trait IntEnum: Sized + Copy {
    /// The name of the matching Python enum, e.g. `"Dash"`.
    const PY_NAME: &'static str;
    /// The Python member names and their values, in declaration order.
    const MEMBERS: &'static [(&'static str, i64)];
    /// The variant with integer value `value`, or `None` for an unknown value.
    fn from_value(value: i64) -> Option<Self>;
    /// The integer value of this variant.
    #[allow(
        dead_code,
        reason = "only the round-trip test of the tables reads values back"
    )]
    fn value_of(self) -> i64;
}

/// Implements [`IntEnum`] for a core enum from one table of
/// `"PYTHON_NAME" => Variant = value` rows.
macro_rules! int_enum {
    ($py_name:literal, $rust:ty { $($member:literal => $variant:path = $value:literal),+ $(,)? }) => {
        impl IntEnum for $rust {
            const PY_NAME: &'static str = $py_name;
            const MEMBERS: &'static [(&'static str, i64)] = &[$(($member, $value)),+];

            fn from_value(value: i64) -> Option<Self> {
                match value {
                    $($value => Some($variant),)+
                    _ => None,
                }
            }

            fn value_of(self) -> i64 {
                match self {
                    $($variant => $value,)+
                }
            }
        }
    };
}

int_enum!("Dash", Dash {
    "SOLID" => Dash::Solid = 0,
    "DASHED" => Dash::Dashed = 1,
    "DOTTED" => Dash::Dotted = 2,
});

int_enum!("Marker", Marker {
    "CIRCLE" => Marker::Circle = 0,
    "SQUARE" => Marker::Square = 1,
    "TRIANGLE" => Marker::Triangle = 2,
    "DIAMOND" => Marker::Diamond = 3,
    "CROSS" => Marker::Cross = 4,
});

int_enum!("Font", Font {
    "SOURCE_SERIF_4" => Font::SourceSerif4 = 0,
    "IBM_PLEX_SANS" => Font::IbmPlexSans = 1,
    "GEIST" => Font::Geist = 2,
    "INTER" => Font::Inter = 3,
    "JETBRAINS_MONO" => Font::JetBrainsMono = 4,
});

int_enum!("LegendStyle", LegendStyle {
    "HEADER" => LegendStyle::Header = 0,
    "DIRECT" => LegendStyle::Direct = 1,
    "CHIPS" => LegendStyle::Chips = 2,
    "NONE" => LegendStyle::None = 3,
});

int_enum!("FillStyle", FillStyle {
    "SOLID" => FillStyle::Solid = 0,
    "GRADIENT" => FillStyle::Gradient = 1,
});

int_enum!("GridLines", GridLines {
    "HORIZONTAL" => GridLines::Horizontal = 0,
    "VERTICAL" => GridLines::Vertical = 1,
    "BOTH" => GridLines::Both = 2,
    "NONE" => GridLines::None = 3,
});

int_enum!("SvgText", SvgText {
    "OUTLINES" => SvgText::Outlines = 0,
    "EDITABLE" => SvgText::Editable = 1,
});

int_enum!("Share", Share {
    "NONE" => Share::None = 0,
    "ROW" => Share::Row = 1,
    "COL" => Share::Col = 2,
    "ALL" => Share::All = 3,
});

int_enum!("Legend", LegendMode {
    "MERGED" => LegendMode::Merged = 0,
    "PER_PANEL" => LegendMode::PerPanel = 1,
});

int_enum!("LegendPosition", LegendPosition {
    "TOP_LEFT" => LegendPosition::TopLeft = 0,
    "TOP_RIGHT" => LegendPosition::TopRight = 1,
    "RIGHT" => LegendPosition::Right = 2,
    "BOTTOM" => LegendPosition::Bottom = 3,
    "BEST" => LegendPosition::Best = 4,
});

int_enum!("Align", Align {
    "LEFT" => Align::Left = 0,
    "CENTER" => Align::Center = 1,
    "RIGHT" => Align::Right = 2,
});

int_enum!("Step", Step {
    "POST" => Step::Post = 0,
    "PRE" => Step::Pre = 1,
    "MID" => Step::Mid = 2,
});

int_enum!("ColorbarPosition", ColorbarPosition {
    "RIGHT" => ColorbarPosition::Right = 0,
    "BOTTOM" => ColorbarPosition::Bottom = 1,
    "NONE" => ColorbarPosition::None = 2,
});

/// How cells are drawn, as the Python `Render` enum names the choices other than `AUTO`,
/// which the bindings pass as `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RenderChoice {
    /// [`Render::Vector`].
    Vector,
    /// [`Render::Raster`].
    Raster,
}

int_enum!("Render", RenderChoice {
    "VECTOR" => RenderChoice::Vector = 0,
    "RASTER" => RenderChoice::Raster = 1,
});

/// The core [`Render`] of a Python `Render` value, `None` standing for `AUTO`.
pub(crate) fn render_choice(value: Option<i64>) -> PyResult<Render> {
    Ok(match value.map(enum_value::<RenderChoice>).transpose()? {
        None => Render::Auto,
        Some(RenderChoice::Vector) => Render::Vector,
        Some(RenderChoice::Raster) => Render::Raster,
    })
}

/// The variant of `E` with integer value `value`; an unknown value is an internal `ValueError`.
pub(crate) fn enum_value<E: IntEnum>(value: i64) -> PyResult<E> {
    E::from_value(value)
        .ok_or_else(|| internal_value(format!("{value} is not a value of {}", E::PY_NAME)))
}

/// Every bound enum that has a public Python enum: its Python name and its `(member name, value)`
/// pairs. The Python-only `Auto` sentinel has no core enum table.
const TABLES: [(&str, &[(&str, i64)]); 14] = [
    (Dash::PY_NAME, Dash::MEMBERS),
    (Marker::PY_NAME, Marker::MEMBERS),
    (Font::PY_NAME, Font::MEMBERS),
    (LegendStyle::PY_NAME, LegendStyle::MEMBERS),
    (FillStyle::PY_NAME, FillStyle::MEMBERS),
    (GridLines::PY_NAME, GridLines::MEMBERS),
    (SvgText::PY_NAME, SvgText::MEMBERS),
    (Share::PY_NAME, Share::MEMBERS),
    (LegendMode::PY_NAME, LegendMode::MEMBERS),
    (LegendPosition::PY_NAME, LegendPosition::MEMBERS),
    (Align::PY_NAME, Align::MEMBERS),
    (Step::PY_NAME, Step::MEMBERS),
    (ColorbarPosition::PY_NAME, ColorbarPosition::MEMBERS),
    (RenderChoice::PY_NAME, RenderChoice::MEMBERS),
];

/// The member tables of every bound enum, keyed by the Python enum name.
#[pyfunction]
pub(crate) fn enum_members(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let tables = PyDict::new(py);
    for (name, members) in TABLES {
        tables.set_item(name, members.to_vec())?;
    }
    Ok(tables)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trips<E: IntEnum + PartialEq + std::fmt::Debug>() {
        for (index, (name, value)) in E::MEMBERS.iter().enumerate() {
            let variant = E::from_value(*value).unwrap();
            assert_eq!(variant.value_of(), *value, "{}.{name}", E::PY_NAME);
            assert_eq!(
                *value,
                i64::try_from(index).unwrap(),
                "{}.{name}",
                E::PY_NAME
            );
        }
        assert_eq!(E::from_value(-1), None);
        assert_eq!(
            E::from_value(i64::try_from(E::MEMBERS.len()).unwrap()),
            None
        );
    }

    #[test]
    fn every_table_round_trips_with_values_in_declaration_order() {
        round_trips::<Dash>();
        round_trips::<Marker>();
        round_trips::<Font>();
        round_trips::<LegendStyle>();
        round_trips::<FillStyle>();
        round_trips::<GridLines>();
        round_trips::<SvgText>();
        round_trips::<Share>();
        round_trips::<LegendMode>();
        round_trips::<LegendPosition>();
        round_trips::<Align>();
        round_trips::<Step>();
        round_trips::<ColorbarPosition>();
        round_trips::<RenderChoice>();
    }

    #[test]
    fn render_choices_map_to_the_core_enum() {
        assert_eq!(render_choice(None).unwrap(), Render::Auto);
        assert_eq!(render_choice(Some(0)).unwrap(), Render::Vector);
        assert_eq!(render_choice(Some(1)).unwrap(), Render::Raster);
    }

    #[test]
    fn tables_are_listed_once_each() {
        let names: Vec<&str> = TABLES.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            names,
            [
                "Dash",
                "Marker",
                "Font",
                "LegendStyle",
                "FillStyle",
                "GridLines",
                "SvgText",
                "Share",
                "Legend",
                "LegendPosition",
                "Align",
                "Step",
                "ColorbarPosition",
                "Render"
            ]
        );
    }
}
