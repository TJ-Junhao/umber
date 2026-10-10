//! Helpers shared by unit tests in different modules. Compiled only for `cargo test`.

pub(crate) mod scene;

// The integration tests include this file by path, so it only uses `std` and the `png` crate.
pub(crate) mod image;
