//! Fitting point-sized bubbles in a continuous domain, before tick rounding and padding.

use crate::error::number;
use crate::scale::log_offset;
use crate::{ContinuousScale, Error, Result, Transform};

/// One visible bubble's coordinate and full radius, attached to its panel's plot length.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Bubble {
    /// The coordinate on this axis.
    pub value: f64,
    /// The path radius plus half the outline width, in points.
    pub radius: f64,
    /// The panel index in row-major order, including panels with hidden tick labels.
    pub panel: usize,
}

/// The raw domain and all bubbles sharing one automatic axis.
pub(crate) struct BubbleFit {
    /// The raw data and explicit tick extent, expanded only if it is a single value.
    pub seed: ContinuousScale,
    /// Every visible circle sharing this scale.
    pub bubbles: Vec<Bubble>,
}

impl BubbleFit {
    /// Whether every circle, including its outline, fits the final scale and plot lengths.
    pub fn contains(&self, scale: ContinuousScale, lengths: &[f64]) -> bool {
        self.bubbles.iter().all(|bubble| {
            let p = scale.normalize(bubble.value);
            let r = bubble.radius / lengths[bubble.panel];
            p >= r && p <= 1.0 - r
        })
    }

    /// The smallest domain enclosing `scale` and every circle. In normalized axis space a
    /// circle at `v` of radius share `a` needs `lo <= v - a*d` and `hi >= v + a*d`.
    /// Thus `d = max(v + a*d) + max(-v + a*d)`. Both maxima are upper envelopes of lines;
    /// sweeping their breakpoints finds the exact active pair, in O(n log n), not all pairs.
    /// Log axes use the same solve in log space. The seed supplies a nonzero singleton range.
    pub fn fit(
        &self,
        scale: ContinuousScale,
        lengths: &[f64],
        context: &'static str,
    ) -> Result<(f64, f64)> {
        let mut upper = vec![Line {
            slope: 0.0,
            intercept: 1.0,
        }];
        let mut lower = vec![Line {
            slope: 0.0,
            intercept: 0.0,
        }];
        for bubble in &self.bubbles {
            let length = lengths[bubble.panel];
            if 2.0 * bubble.radius >= length {
                return Err(Error::style(context, format!(
                    "a bubble is {} pt across including its outline; the plot is {} pt long on this axis.",
                    number(2.0 * bubble.radius), number(length),
                )).with_hint("make the figure larger, or set explicit limits to allow clipping."));
            }
            // A small drawing-space allowance covers the final f32 path coordinates.
            let slope = (bubble.radius + 1e-4).min((bubble.radius + length / 2.0) / 2.0) / length;
            let value = scale.normalize(bubble.value);
            upper.push(Line {
                slope,
                intercept: value,
            });
            lower.push(Line {
                slope,
                intercept: -value,
            });
        }
        let upper = envelope(upper);
        let lower = envelope(lower);
        let (mut i, mut j) = (0, 0);
        let (lo, hi) = loop {
            let (a, b) = (upper[i].0, lower[j].0);
            let span = (a.intercept + b.intercept) / (1.0 - a.slope - b.slope);
            let next_upper = upper.get(i + 1).map_or(f64::INFINITY, |line| line.1);
            let next_lower = lower.get(j + 1).map_or(f64::INFINITY, |line| line.1);
            if span <= next_upper.min(next_lower) {
                break (
                    -(b.intercept + b.slope * span),
                    a.intercept + a.slope * span,
                );
            }
            if next_upper <= next_lower {
                i += 1;
            }
            if next_lower <= next_upper {
                j += 1;
            }
        };
        let (low, high) = scale.domain();
        let value = |p: f64| match scale.transform() {
            Transform::Linear => low + p * (high - low),
            Transform::Log(_) => {
                let offset = p * log_offset(high, low);
                let scaled = low * offset.exp();
                if scaled.is_normal() {
                    scaled
                } else {
                    (low.ln() + offset).exp()
                }
            }
        };
        let domain = (value(lo).min(low), value(hi).max(high));
        ContinuousScale::new(scale.transform(), domain.0, domain.1, context)
            .map(|scale| scale.domain())
            .map_err(|_| Error::style(context,
                "automatic limits cannot contain every bubble within the supported numeric range.")
                .with_hint("move values away from the numeric bounds, or set explicit limits to allow clipping."))
    }
}

/// A line of the upper or reflected lower envelope, in normalized axis space.
#[derive(Debug, Clone, Copy)]
struct Line {
    slope: f64,
    intercept: f64,
}

/// Upper envelope in increasing slope order, paired with each line's first active span.
/// Equal slopes keep only the largest intercept; no division by zero or quadratic search.
fn envelope(mut lines: Vec<Line>) -> Vec<(Line, f64)> {
    lines.sort_by(|a, b| {
        a.slope
            .total_cmp(&b.slope)
            .then(a.intercept.total_cmp(&b.intercept))
    });
    let mut hull: Vec<(Line, f64)> = Vec::new();
    for line in lines {
        let start = loop {
            let Some(&(last, start)) = hull.last() else {
                break f64::NEG_INFINITY;
            };
            if line.slope == last.slope {
                hull.pop();
                continue;
            }
            let crossing = (last.intercept - line.intercept) / (line.slope - last.slope);
            if crossing <= start {
                hull.pop();
            } else {
                break crossing;
            }
        };
        hull.push((line, start));
    }
    hull
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_diameter_and_unrepresentable_domains_are_errors() {
        let seed = ContinuousScale::new(Transform::Linear, 0.0, 1.0, "test").unwrap();
        let fit = BubbleFit {
            seed,
            bubbles: vec![Bubble {
                value: 0.5,
                radius: 18.4,
                panel: 0,
            }],
        };
        assert!(fit.fit(seed, &[36.8], "test").is_err());
        for (transform, low, high, value) in [
            (
                Transform::Linear,
                0.0,
                crate::MAX_MAGNITUDE,
                crate::MAX_MAGNITUDE,
            ),
            (
                Transform::Log(crate::LogBase::TEN),
                crate::MIN_LOG_VALUE,
                1.0,
                crate::MIN_LOG_VALUE,
            ),
        ] {
            let seed = ContinuousScale::new(transform, low, high, "test").unwrap();
            let fit = BubbleFit {
                seed,
                bubbles: vec![Bubble {
                    value,
                    radius: 18.4,
                    panel: 0,
                }],
            };
            let error = fit.fit(seed, &[200.0], "test").unwrap_err();
            assert!(error.to_string().contains("supported numeric range"));
            assert!(error.hint().is_some());
        }
    }

    #[test]
    fn direct_fit_matches_all_pair_constraints() {
        let seed = ContinuousScale::new(Transform::Linear, 0.0, 1.0, "test").unwrap();
        let bubbles: Vec<_> = (0..101)
            .map(|i| Bubble {
                value: f64::from(i) / 100.0,
                radius: f64::from((i * 37) % 49),
                panel: 0,
            })
            .collect();
        let fit = BubbleFit { seed, bubbles };
        let (lo, hi) = fit.fit(seed, &[100.0], "test").unwrap();
        let mut points = vec![(0.0, 0.0), (1.0, 0.0)];
        points.extend(
            fit.bubbles
                .iter()
                .map(|b| (b.value, (b.radius + 1e-4) / 100.0)),
        );
        let mut expected: f64 = 1.0;
        for &(a, ra) in &points {
            for &(b, rb) in &points {
                expected = expected.max((b - a) / (1.0 - ra - rb));
            }
        }
        assert!((hi - lo - expected).abs() < 1e-12);
        assert!(fit.contains(
            ContinuousScale::new(Transform::Linear, lo, hi, "test").unwrap(),
            &[100.0]
        ));
    }
}
