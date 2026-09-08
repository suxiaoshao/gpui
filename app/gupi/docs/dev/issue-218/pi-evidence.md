# Pi 进程与窗口：证据及阶段边界

所属 [应用计划](README.md)，状态见[根计划](../../../../../docs/dev/issue-218/README.md)。本轮仅阅读源码，不运行 Pi、不安装参考项目依赖。

## 官方证据

本地 Pi：`/Users/sushao/Documents/code/pi`，提交 `da840b6216578c2a571d0374ac6a2091a83f9d91`。

- [官方 RPC 文档](https://pi.dev/docs/latest/rpc)：原生宿主可以通过 `pi --mode rpc` 的 stdin/stdout JSONL 集成；Node/TypeScript 应用另外可直接采用 AgentSession。Gupi 是 Rust 宿主，保留已选定的 CLI 子进程边界。
- [官方 rpc-client.ts](https://github.com/earendil-works/pi/blob/da840b6216578c2a571d0374ac6a2091a83f9d91/packages/coding-agent/src/modes/rpc/rpc-client.ts)：start 使用 node+CLI、独立管道、cwd/env，监听退出与 stdin 错误，退出后拒绝挂起请求；stop 发送 SIGTERM，1 秒后尝试 SIGKILL。这是 TypeScript 客户端实现证据，不是所有平台的强制推荐时限，也未证明强杀后已完成 wait。
- [官方 rpc-mode.ts](https://github.com/earendil-works/pi/blob/da840b6216578c2a571d0374ac6a2091a83f9d91/packages/coding-agent/src/modes/rpc/rpc-mode.ts)：stdin end 调用 shutdown；shutdown dispose runtimeHost 后退出。SIGTERM/SIGHUP 路径还会清理跟踪的 detached 子进程。不能仅把 abort 命令当作进程退出。
- RPC 文档明确 abort 后仍可能继续排队消息；交互式停止另涉及 clear_queue。第一阶段不发送这些命令，第二阶段必须区分停止本轮执行与关闭进程。

## GUI 实现参考

已克隆 `StarkInternationalAI/pi-desktop` 到 `/tmp/gupi-reference-pi-desktop-20260907`，提交 `7ffbc1606475a22bfbcec4252ab0577b821305ff`。选取理由是 Rust/Tauri 后端实际包装本机 Pi RPC，与 Gupi 边界接近；没有核实其用户规模，不称其为业界标准。

[process_manager.rs](https://github.com/StarkInternationalAI/pi-desktop/blob/7ffbc1606475a22bfbcec4252ab0577b821305ff/src-tauri/src/process_manager.rs) 持有 stdin writer、stdout reader、退出监听任务，并以 abort、关闭 stdin、延迟 SIGTERM 的顺序关闭。参考其职责拆分；不照抄固定 sleep 后按 PID 发信号、随后 abort 退出监听的方式，Gupi 需要持有 child 并确认 wait 结果。

另查看 [justhil/pi-app](https://github.com/justhil/pi-app) 项目说明：它采用 Pi SDK。可参考产品交互，但该集成边界不能替代本机 CLI 进程管理契约；本轮未审查其进程代码。

## 第一阶段采用范围

版本检查只执行已选命令加独立 `--version` 参数，使用 15 秒整体 deadline（包含排队、查找、启动和读取输出）与每路 16 KiB 输出限制；stdout/stderr 并发读取，结束后回收 child。超时或用户取消是结束探测的原因，不涉及保存对话。版本探测不发送 abort、不建立 RPC 会话，不等待模型或工具结束。

查找和启动在有两项并发上限的后台阻塞任务执行；15 秒到期停止等待。Operation Cancel 丢弃 GPUI/Tokio 完成链，关闭结果接收通道；阻塞任务不能强制中断，需检查取消后再启动，并释放无法交接的迟到 Child。Child 的 kill_on_drop 触发终止，取消不承诺此刻已完成操作系统回收。正常完成显式 wait；失败后已持有 Child 的 kill/wait 额外限 2 秒，异常收尾写日志并 Drop 兜底。详细状态及资源归属见运行时 L-112，不借此加入 RPC 生命周期实现。

Windows 的 npm shim 与 Unix 可执行脚本不同，尚未验证的支持不宣称完成。官方 TS 客户端使用 node+CLI 不能直接解释成 Gupi 有权绕过用户配置的 pi 命令。按用户命令启动、不自动修改 PATH 或安装 Node 的边界保持不变。

## 第二阶段保留证据

长期 RPC 的优雅退出、挂起请求失败、工具后代进程收尾、关闭 stdin 与信号升级顺序在 #219 定稿。官方已有 EOF/shutdown 与信号处理，可据此设计有界收尾；不为第一阶段提前实现，也不把“模型迟迟不结束”列为 #218 的保存阻断。

## 窗口恢复

Pi 不负责 GUI 窗口布局。沿用 Jaco `app/jaco/src/state/layout.rs` 的 PersistedWindowBounds 转换思路：记录 GPUI Pixels 对应的 x/y/width/height、窗口模式和显示器标识，再恢复 WindowBounds；不可把坐标误当作硬件物理像素。Gupi 本阶段只取普通/最大化需求和离屏处理，不复制 Jaco 多窗口与业务状态。窗口恢复不读取 Pi 会话文件。
