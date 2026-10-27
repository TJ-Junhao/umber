//! The axes-wide bubble area scale and its reference values.

/// A radius in points; zero for a NaN, zero or negative size. Taking square roots separately avoids underflow in the ratio.
pub(crate) fn radius(value: f64, maximum: f64, radius: f32) -> f32 {
    // A NaN size, which leaves its bubble out, has no radius.
    if value.is_nan() || value <= 0.0 || maximum <= 0.0 {
        0.0
    } else {
        (f64::from(radius) * (value.sqrt() / maximum.sqrt())) as f32
    }
}

/// Nearby finite positive members of the decimal 1-2-5 sequence.
fn candidates(value: f64) -> impl Iterator<Item = f64> {
    let exponent = value.log10().floor() as i32;
    (exponent - 1..=exponent + 1)
        .flat_map(|e| [1, 2, 5].map(move |m| (m, e)))
        // Decimal parsing also handles subnormal values, where 10^e alone can underflow.
        .filter_map(|(m, e)| format!("{m}e{e}").parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
}

/// The largest 1-2-5 value no greater than `value`; zero for an empty scale.
pub(crate) fn nice_125_below(value: f64) -> f64 {
    if value <= 0.0 || !value.is_finite() {
        return 0.0;
    }
    candidates(value)
        .filter(|v| *v <= value)
        .fold(0.0, f64::max)
}

/// The nearest 1-2-5 value in logarithmic distance; zero for an empty scale.
pub(crate) fn nice_125_round(value: f64) -> f64 {
    if value <= 0.0 || !value.is_finite() {
        return 0.0;
    }
    let log = value.ln();
    candidates(value)
        .min_by(|a, b| (a.ln() - log).abs().total_cmp(&(b.ln() - log).abs()))
        .unwrap_or(0.0)
}

/// Up to three distinct positive reference values, largest first; none for an all-zero scale.
pub(crate) fn references(maximum: f64) -> Vec<f64> {
    let top = nice_125_below(maximum);
    let mut values: Vec<_> = [top, nice_125_round(top / 2.0), nice_125_round(top / 4.0)]
        .into_iter()
        .filter(|v| *v > 0.0)
        .collect();
    values.dedup();
    values
}
