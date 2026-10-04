# feiwen-data

数据库、小说数据模型、查询和目录服务。应用通过 init_store 注入数据库路径；DatabaseStore 和 CatalogStore 保留各自资源生命周期，不依赖窗口或功能视图。

```sh
cargo test -p feiwen-data
```
