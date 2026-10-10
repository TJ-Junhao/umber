# umber

> **Pre-alpha 预览版。** API 不稳定，随时可能变更且不另行通知。本项目尚不可用。

umber 是一个用 Rust 编写的 Python 绘图库。渲染核心作为独立的 `umber-core` crate 发布。

目前 `pip install umber` 只提供包的骨架，还不能绘图。

## 安装

```sh
pip install umber
```

目前仅提供 macOS Apple silicon（arm64）的预编译 wheel。其他平台上 `pip` 会从源码构建，需要 Rust 1.88 或更新版本以及 C 链接器。

## 许可证

以 MIT 或 Apache-2.0 双许可（`MIT OR Apache-2.0`），任选其一。内置字体采用 SIL Open Font License 1.1，参见 `crates/umber-core/fonts/LICENSES` 与 `crates/umber-core/fonts/MANIFEST.md`。发布的 wheel 包含静态链接的 Rust 依赖，其许可证列于 `THIRD-PARTY-LICENSES.txt`。
