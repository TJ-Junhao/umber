//! Helpers shared by unit tests in different modules. Compiled only for `cargo test`.

pub(crate) mod scene;

// The same decoder is used by the integration tests, so it lives under `tests/support`.
#[path = "../../tests/support/image.rs"]
pub(crate) mod image;
