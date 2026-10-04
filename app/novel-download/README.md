# Novel Download

小说下载工具。

```sh
cargo run -p novel-download
cargo test -p novel-download-feature -p novel-download
```

`src/` 初始化窗口、日志和本地化，向 [下载功能](crates/novel-download-feature/README.md) 注入系统下载目录。功能拥有表单、源解析、后端、运行状态、取消和暂存输出；应用保持原有启动和打包入口。
