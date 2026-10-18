//! A finished scene keeps the system fonts its text was shaped with: its PDF does not change
//! when a font file is replaced or removed afterwards.
//!
//! The test runs itself again in a child process whose fontconfig sees only one test font in a
//! scratch directory, so the system font index of the child holds that font alone.

// clippy's allow-in-tests settings cover only `#[test]` functions, not the helpers here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg(target_os = "linux")]

use std::path::{Path as FsPath, PathBuf};
use std::process::Command;

use umber_core::render::render_pdf;
use umber_core::scene::Scene;
use umber_core::{
    FaceId, Figure, Font, FontLibrary, Point, Rgba, ScaleKind, TextStyle, Theme, Weight,
};

#[path = "support/pdf_check.rs"]
mod pdf_check;

use pdf_check::{assert_readers_parse, extracted_text, have, scratch, write};

/// Set in the child process to the scratch directory holding its fonts.
const CHILD: &str = "UMBER_PDF_SYSTEM_FONT_DIR";
const TEST: &str = "a_finished_scene_keeps_its_system_fonts";
/// The characters only the test font draws.
const TEXT: &str = "A\u{E001}\u{E002}";

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name),
    )
    .unwrap()
}

#[test]
fn a_finished_scene_keeps_its_system_fonts() {
    if let Some(dir) = std::env::var_os(CHILD) {
        child(&PathBuf::from(dir));
        return;
    }
    let dir = scratch("system-fonts");
    let fonts = dir.join("fonts");
    std::fs::create_dir_all(&fonts).unwrap();
    std::fs::write(fonts.join("test-font.otf"), fixture("outline-cff.otf")).unwrap();
    let config = dir.join("fonts.conf");
    std::fs::write(
        &config,
        format!(
            "<?xml version=\"1.0\"?>\n<fontconfig><dir>{}</dir></fontconfig>\n",
            fonts.display()
        ),
    )
    .unwrap();
    let out = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", TEST, "--nocapture", "--test-threads", "1"])
        .env(CHILD, &dir)
        .env("FONTCONFIG_FILE", &config)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "child failed:\n{stdout}\n{stderr}");
    assert!(
        stdout.contains("child passed"),
        "the child did not run:\n{stdout}\n{stderr}"
    );
}

/// Renders scenes shaped with the test font, then replaces the font file with another font
/// and removes it, rendering the same scenes again each time.
fn child(dir: &FsPath) {
    let font_file = dir.join("fonts").join("test-font.otf");
    let library = FontLibrary::bundled();
    let fonts = library.with_figure_fonts(&[Font::INTER]).unwrap();
    let style = TextStyle {
        face: FaceId::new(0),
        weight: Weight::Regular,
        size: 18.0,
        tabular_figures: false,
    };
    let white = Rgba {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    let mut scene = Scene::new(200.0, 60.0, white);
    scene
        .push_text(
            &fonts,
            TEXT,
            style,
            Point { x: 10.0, y: 40.0 },
            Rgba {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
        )
        .unwrap();
    let mut figure = Figure::new(3.0, 2.0, Theme::pigment()).unwrap();
    figure
        .add_axes(ScaleKind::Linear, ScaleKind::Linear)
        .unwrap()
        .title(TEXT);
    let figure_scene = figure.to_scene(&library).unwrap();

    let before = [
        render_pdf(&scene).unwrap(),
        render_pdf(&figure_scene).unwrap(),
    ];
    for pdf in &before {
        assert!(
            String::from_utf8_lossy(pdf).contains("+UmberTestCFF-Regular"),
            "the text is not set in the test font"
        );
    }
    // Overwritten in place with a different font.
    std::fs::write(&font_file, fixture("outline-variable.ttf")).unwrap();
    let replaced = [
        render_pdf(&scene).unwrap(),
        render_pdf(&figure_scene).unwrap(),
    ];
    std::fs::remove_file(&font_file).unwrap();
    let removed = [
        render_pdf(&scene).unwrap(),
        render_pdf(&figure_scene).unwrap(),
    ];
    assert!(
        before == replaced,
        "replacing the font file changed the PDF"
    );
    assert!(before == removed, "removing the font file changed the PDF");

    let path = write(dir, "removed.pdf", &removed[0]);
    assert_readers_parse(&path);
    if have("pdftotext") {
        let text = extracted_text(&path);
        assert!(text.contains(TEXT), "{text:?}");
    }
    println!("child passed");
}
