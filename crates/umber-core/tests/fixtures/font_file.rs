//! Optional OFL fixtures that live outside the published crate.

#![allow(clippy::panic)]

use std::path::Path;

pub fn read(name: &str) -> Option<Vec<u8>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = root.join("tests/fonts");
    let path = directory.join(name);
    match std::fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && !directory.try_exists().unwrap()
                && !root.join(".git").try_exists().unwrap() =>
        {
            eprintln!(
                "skipping font fixture {name}: tests/fonts is absent outside a repository checkout"
            );
            None
        }
        Err(error) => panic!("cannot read font fixture {}: {error}", path.display()),
    }
}
