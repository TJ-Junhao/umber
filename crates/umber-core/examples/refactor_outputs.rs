//! Writes every output of the refactor corpus into a directory, for the output gate.
//!
//! ```text
//! cargo run --release --locked -p umber-core --example refactor_outputs -- <output-dir>
//! ```
//!
//! The directory gets `scene/`, `svg/`, `png/`, `errors/` and `api/` subdirectories, one file
//! per output. `scripts/check-refactor-output.sh` hashes them and compares manifests; run it
//! instead of this example directly.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../tests/support/refactor_cases.rs"]
mod refactor_cases;

use std::path::PathBuf;

fn main() {
    let Some(dir) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: refactor_outputs <output-dir>");
        std::process::exit(2);
    };
    let mut count = 0usize;
    refactor_cases::generate(|output| {
        // The gate's manifests are read as "<hash> <path>", so names must not contain whitespace.
        assert!(
            !output.file.chars().any(char::is_whitespace),
            "output file name contains whitespace: {:?}",
            output.file
        );
        let kind_dir = dir.join(output.kind);
        std::fs::create_dir_all(&kind_dir).unwrap();
        std::fs::write(kind_dir.join(&output.file), &output.bytes).unwrap();
        count += 1;
    });
    eprintln!("wrote {count} files to {}", dir.display());
}
