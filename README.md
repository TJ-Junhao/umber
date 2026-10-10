# umber

> **Pre-alpha preview.** The API is unstable and will change without notice. This project is not ready for use.

umber is a plotting library for Python, written in Rust. The rendering core is published separately as the `umber-core` crate.

`pip install umber` currently provides only the package skeleton. There is no plotting yet.

## Installation

```sh
pip install umber
```

Prebuilt wheels are currently provided for macOS on Apple silicon (arm64) only. On other platforms, `pip` builds umber from source, which requires Rust 1.88 or newer and a C linker.

## License

Licensed under either of MIT or Apache-2.0, at your option (`MIT OR Apache-2.0`). Bundled fonts are under the SIL Open Font License 1.1; see `crates/umber-core/fonts/LICENSES` and `crates/umber-core/fonts/MANIFEST.md`. Wheels include statically linked Rust dependencies; their licenses are listed in `THIRD-PARTY-LICENSES.txt`.
