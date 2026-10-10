//! Writes every output of the refactor corpus into a directory, for the output gate.
//!
//! ```text
//! cargo run --release --locked -p umber-core --example refactor_outputs -- <output-dir>
//! ```
//!
//! The directory gets `scene/`, `svg/`, `png/`, `errors/` and `api/` subdirectories, one file
//! per output. `scripts/check-refactor-output.sh` hashes them and compares manifests; run it
//! instead of this example directly. Every output name must follow the naming rule in that
//! script's header, and the example refuses to write anything if one does not.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../tests/support/refactor_cases.rs"]
mod refactor_cases;

use std::path::PathBuf;

/// The directories an output may go to.
const KINDS: [&str; 5] = ["scene", "svg", "png", "errors", "api"];

/// Whether `kind` and `file` follow the naming rule of the gate: a known kind, and a file name of
/// ASCII letters, digits, '.', '_' and '-' that starts with a letter or digit. Such a name has no
/// path separator, whitespace or '#', so manifests can be read back and cases can be listed.
fn valid_name(kind: &str, file: &str) -> bool {
    let mut chars = file.chars();
    KINDS.contains(&kind)
        && chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn main() {
    let Some(dir) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("usage: refactor_outputs <output-dir>");
        std::process::exit(2);
    };
    // Every name is checked before anything is written.
    let mut outputs = Vec::new();
    refactor_cases::generate(|output| outputs.push(output));
    let bad: Vec<String> = outputs
        .iter()
        .filter(|o| !valid_name(o.kind, &o.file))
        .map(|o| format!("{}/{}", o.kind, o.file))
        .collect();
    if !bad.is_empty() {
        eprintln!("output names break the naming rule (see scripts/check-refactor-output.sh):");
        for name in &bad {
            eprintln!("  {name:?}");
        }
        std::process::exit(1);
    }
    for output in &outputs {
        let kind_dir = dir.join(output.kind);
        std::fs::create_dir_all(&kind_dir).unwrap();
        std::fs::write(kind_dir.join(&output.file), &output.bytes).unwrap();
    }
    eprintln!("wrote {} files to {}", outputs.len(), dir.display());
}

#[cfg(test)]
mod tests {
    use super::valid_name;

    #[test]
    fn accepts_the_names_the_corpus_uses() {
        assert!(valid_name("scene", "fig-a_1.txt"));
        assert!(valid_name("svg", "fig-a.editable.svg"));
        assert!(valid_name("png", "fig-a.dpi72.png"));
        assert!(valid_name("errors", "api.txt"));
        assert!(valid_name("api", "constants-bars.txt"));
    }

    #[test]
    fn rejects_names_outside_the_rule() {
        assert!(!valid_name("other", "a.txt"));
        assert!(!valid_name("scene", "errors/api.txt"));
        assert!(!valid_name("scene", "a b.txt"));
        assert!(!valid_name("scene", "a\u{a0}b.txt"));
        assert!(!valid_name("scene", "a\nb.txt"));
        assert!(!valid_name("scene", "a#b.txt"));
        assert!(!valid_name("scene", "\u{e9}.txt"));
        assert!(!valid_name("scene", ".hidden.txt"));
        assert!(!valid_name("scene", ""));
    }
}
