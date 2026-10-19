//! Every renderable corpus scene is a valid PDF and agrees with its PNG at both resolutions.
//!
//! The committed manifest stores PNG hashes, not image files. The shared generator recreates
//! those PNGs; the output gate checks their byte identity on the reference machine. PDFs are
//! temporary test files and never enter the manifest. Run with `--nocapture` for the full table.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "support/pdf_check.rs"]
mod pdf_check;
#[path = "support/refactor_cases.rs"]
mod refactor_cases;

use std::collections::BTreeSet;

// Both corpus resolutions: 4.0 mean / 2.20% far were the largest ordinary differences.
// The corpus includes smaller canvases and denser edges at 72/200 dpi than the focused
// tests at 144 dpi, so it needs 4.5 / 2.5% rather than pdf_check's 2.0 / 1% defaults.
// Keep margin for rasterizer versions without raising every scene's limits to those of
// the densely packed, subpixel-sized stress cases.
const MAX_MEAN_DIFFERENCE: f64 = 4.5;
const MAX_FAR_SHARE: f64 = 0.025;

// Poppler and MuPDF limits for the same nine scenes. The largest MuPDF differences at
// 72/200 dpi are 13.08 mean and 12.60% far; most are much closer than Poppler.
// Both rasterizers agree on the geometry, but not on coverage at tiny edges. The generated
// three-way table records both PNG comparisons and the PDF rasterizers' disagreement.
fn exceptional_tolerances(name: &str) -> Option<((f64, f64), (f64, f64))> {
    Some(match name {
        // Poppler snaps dense, subpixel-width bars more opaquely than tiny-skia or MuPDF.
        // A broad Poppler limit must not conceal a change shared by both PDF rasterizers.
        "fig-hist-max-bins-graphite" => ((32.0, 0.24), (16.0, 0.16)),
        "fig-hist-max-bins-midnight" => ((12.0, 0.11), (5.0, 0.008)),
        "fig-category-labels-bars-200-horizontal" => ((21.0, 0.22), (6.0, 0.0001)),
        "fig-category-labels-bars-200-vertical" => ((17.0, 0.17), (4.5, 0.0001)),
        // Dense overlapping markers amplify differences at their antialiased borders.
        "fig-huge-data" => ((12.0, 0.05), (6.0, 0.01)),
        // Text and line edges occupy a large fraction of these very small canvases.
        "fig-direct-tiny-50-lines" => ((11.0, 0.075), (6.0, 0.016)),
        "fig-squeezed-layout-no-x-ticks" => ((7.0, 0.04), (3.0, 0.013)),
        "fig-legend-narrow-50-entries" => ((6.0, 0.03), (4.0, 0.008)),
        "fig-legend-chips-narrow-50-entries" => ((6.0, 0.03), (5.0, 0.008)),
        _ => return None,
    })
}

fn tolerance(name: &str) -> (f64, f64) {
    exceptional_tolerances(name)
        .map_or((MAX_MEAN_DIFFERENCE, MAX_FAR_SHARE), |(poppler, _)| poppler)
}

fn mupdf_tolerance(name: &str) -> Option<(f64, f64)> {
    exceptional_tolerances(name).map(|(_, mupdf)| mupdf)
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "runs the full corpus; needs the release build"
)]
fn every_corpus_pdf_opens_and_matches_png() {
    let qpdf = pdf_check::have("qpdf");
    let poppler = pdf_check::have("pdftoppm");
    // Check this even if another reader is missing, so required tools cannot silently skip.
    let mupdf = pdf_check::have("mutool");
    let dir = pdf_check::scratch("corpus");
    let manifest = include_str!("refactor/baseline-manifest.txt");
    let expected: BTreeSet<&str> = manifest
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .filter_map(|path| path.strip_prefix("scene/"))
        .map(|file| file.strip_suffix(".txt").unwrap())
        .collect();
    let mut seen = BTreeSet::new();
    let mut rejected = BTreeSet::new();
    let mut failed_cases = BTreeSet::new();
    let mut differences = Vec::new();
    let mut exceptions = Vec::new();
    let mut failures = Vec::new();
    eprintln!("case\tdpi\tmean_channel_difference\tpixels_over_64_percent");
    refactor_cases::generate_pdf(|output| match output.kind {
        "pdf" => {
            let name = output.file.strip_suffix(".pdf").unwrap();
            assert!(seen.insert(name.to_owned()), "duplicate PDF: {name}");
            if !output.bytes.starts_with(b"%PDF-") {
                assert!(
                    matches!(name, "scene-zero-size" | "scene-nan-size"),
                    "{name}: {}",
                    String::from_utf8_lossy(&output.bytes)
                );
                rejected.insert(name.to_owned());
                return;
            }
            assert!(output.bytes.ends_with(b"%%EOF"));
            let case_dir = dir.join(name);
            std::fs::create_dir(&case_dir).unwrap();
            let path = pdf_check::write(&case_dir, "figure.pdf", &output.bytes);
            if qpdf && std::panic::catch_unwind(|| pdf_check::assert_qpdf_clean(&path)).is_err() {
                failed_cases.insert(name.to_owned());
                failures.push(format!(
                    "{name}: qpdf rejected the document; see validator output"
                ));
            }
            if mupdf && std::panic::catch_unwind(|| pdf_check::assert_mupdf_clean(&path)).is_err() {
                failed_cases.insert(name.to_owned());
                failures.push(format!(
                    "{name}: MuPDF rejected the document; see reader output"
                ));
            }
        }
        "png" => {
            let (name, suffix) = output.file.rsplit_once(".dpi").unwrap();
            if rejected.contains(name) {
                // PNG clamps invalid page sizes to a pixel; PDF must reject such pages.
                return;
            }
            assert!(output.bytes.starts_with(b"\x89PNG"), "{name}");
            let dpi = suffix.strip_suffix(".png").unwrap().parse().unwrap();
            if poppler {
                let pdf = dir.join(name).join("figure.pdf");
                let png = pdf_check::png_raster(&output.bytes);
                let comparison = std::panic::catch_unwind(|| {
                    let poppler = pdf_check::poppler_raster_for_png(&pdf, dpi, &png);
                    let difference = pdf_check::raster_difference(&poppler, &png);
                    let extra = if mupdf && mupdf_tolerance(name).is_some() {
                        let mupdf = pdf_check::mupdf_raster_for_png(&pdf, dpi, &png);
                        Some((
                            pdf_check::raster_difference(&mupdf, &png),
                            pdf_check::raster_difference(&poppler, &mupdf),
                        ))
                    } else {
                        None
                    };
                    (difference, extra)
                });
                let row = match comparison {
                    Ok(((mean, far), extra)) => {
                        let (max_mean, max_far) = tolerance(name);
                        if mean > max_mean || far > max_far {
                            failed_cases.insert(name.to_owned());
                            failures.push(format!(
                                "{name} at {dpi} dpi: {mean:.6}, {:.6}%",
                                far * 100.0
                            ));
                        }
                        if let Some(((mu_mean, mu_far), (between_mean, between_far))) = extra {
                            let (max_mu_mean, max_mu_far) = mupdf_tolerance(name).unwrap();
                            if mu_mean > max_mu_mean || mu_far > max_mu_far {
                                failed_cases.insert(name.to_owned());
                                failures.push(format!(
                                    "{name} at {dpi} dpi: MuPDF {mu_mean:.6}, {:.6}%",
                                    mu_far * 100.0
                                ));
                            }
                            exceptions.push(format!(
                                "{name}\t{dpi}\t{mean:.6}\t{:.6}\t{mu_mean:.6}\t{:.6}\t{between_mean:.6}\t{:.6}",
                                far * 100.0, mu_far * 100.0, between_far * 100.0
                            ));
                        }
                        format!("{name}\t{dpi}\t{mean:.6}\t{:.6}", far * 100.0)
                    }
                    Err(_) => {
                        failed_cases.insert(name.to_owned());
                        failures.push(format!("{name} at {dpi} dpi: raster comparison failed"));
                        format!("{name}\t{dpi}\tNA\tNA")
                    }
                };
                eprintln!("{row}");
                differences.push(row);
                if failed_cases.contains(name) || mupdf_tolerance(name).is_some() {
                    pdf_check::write(
                        &dir.join(name),
                        &format!("reference-{dpi}.png"),
                        &output.bytes,
                    );
                    let ppm = pdf.with_extension("ppm");
                    if ppm.is_file() {
                        std::fs::rename(ppm, dir.join(name).join(format!("poppler-{dpi}.ppm")))
                            .unwrap();
                    }
                }
            }
            // Keep failures and the nine exceptions, so their tolerances remain inspectable.
            if dpi == *refactor_cases::DPIS.last().unwrap()
                && !failed_cases.contains(name)
                && mupdf_tolerance(name).is_none()
            {
                std::fs::remove_dir_all(dir.join(name)).unwrap();
            }
        }
        _ => {}
    });
    let rejected_rows: Vec<String> = rejected
        .iter()
        .flat_map(|name| refactor_cases::DPIS.map(|dpi| format!("{name}\t{dpi}\tNA\tNA")))
        .collect();
    for row in &rejected_rows {
        eprintln!("{row}");
    }
    let table = format!(
        "case\tdpi\tmean_channel_difference\tpixels_over_64_percent\n{}\n{}\n",
        differences.join("\n"),
        rejected_rows.join("\n")
    );
    let report = pdf_check::write(&dir, "differences.tsv", table.as_bytes());
    eprintln!("PDF comparison table: {}", report.display());
    let table = format!(
        "case\tdpi\tpoppler_png_mean\tpoppler_png_far_percent\tmupdf_png_mean\tmupdf_png_far_percent\tpoppler_mupdf_mean\tpoppler_mupdf_far_percent\n{}\n",
        exceptions.join("\n")
    );
    let report = pdf_check::write(&dir, "three-way.tsv", table.as_bytes());
    eprintln!("PDF exception table: {}", report.display());
    if poppler && mupdf {
        assert_eq!(exceptions.len(), 9 * refactor_cases::DPIS.len());
    }
    assert_eq!(
        seen.iter().map(String::as_str).collect::<BTreeSet<_>>(),
        expected
    );
    assert_eq!(
        rejected,
        BTreeSet::from(["scene-zero-size".to_owned(), "scene-nan-size".to_owned()])
    );
    if poppler {
        assert_eq!(
            differences.len(),
            (expected.len() - rejected.len()) * refactor_cases::DPIS.len()
        );
    }
    assert!(
        failures.is_empty(),
        "PDF corpus failures:\n{}",
        failures.join("\n")
    );
}
