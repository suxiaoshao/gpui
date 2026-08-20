# Lestty

Lestty 计划作为一个快速、轻量的终端应用。目前仅建立 Rust 二进制 crate，并接入 workspace；尚未添加 GPUI、终端引擎或其他产品实现。

## 当前结构

```text
app/lestty/
├─ Cargo.toml
└─ src/main.rs
```

## 验证

从 workspace 根目录执行：

```bash
cargo check -p lestty --locked
cargo test -p lestty --locked
```
