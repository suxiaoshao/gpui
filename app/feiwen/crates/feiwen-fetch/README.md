# feiwen-fetch

FetchView 聚合表单、网络抓取、解析、写入、运行状态及日志。内部 FetchRun 不跨 crate 暴露；shell 使用摘要和命令接口。订阅和运行任务由抓取 owner 持有。

```sh
cargo test -p feiwen-fetch
```
