# Issue #200：HTTP Client Response 媒体与 PDF 预览发行交付

## 状态与范围

- 状态：`In progress`
- 关联 issue：[#200](https://github.com/suxiaoshao/gpui/issues/200)
- Plan ID：`issue-200`
- 关联前序：[#199](https://github.com/suxiaoshao/gpui/issues/199) 的媒体/PDF 实施记录已移交；其
  [历史计划](../../../app/http-client/docs/dev/issue-199/response-media-and-pdf-preview-plan.md) 为
  `Superseded`，只保存已进入代码的实现与自动化证据。
- Affected owners：`app/http-client`、`crates/xtask`、root 的 `script/` 与 `.github/workflows/`。
- Release gate：`C-2001`。未形成经审计的 native runtime、license/notice、fixture、安装包和三平台
  真实验证证据前，音频/视频预览不得宣称可发行。

本计划只承接 #199 中尚未闭环的发行交付。已有 Response asset、媒体/PDF 私有运行态、viewer、i18n
和定向自动化保持为既有基线，不在本计划重写其实现契约或扩展产品功能。

### 目标

让 HTTP Client 的既有 Response 音频、视频与 PDF 只读预览在受支持平台具备可审计、可打包、可验证的
发行条件。

### 非目标

- 不修改 `RequestRuntime`、transport、Response 收集、Store、History、多 tab 或 Repair。
- 不增加第二套媒体后端、直接 FFmpeg API、媒体编辑、自动播放、PDF zoom 或连续阅读。
- 不把开发 SDK 检查或 Cargo 自动化当作安装包、codec 或手工 UI 验证的替代物。

## 计划地图

| Scope | 文档 | 负责范围 | Work packages |
| --- | --- | --- | --- |
| Root hub | 本文 | `C-2001`、跨 owner 顺序、CI/三平台发行门禁与汇总证据 | `WP-2000`、`WP-2003` |
| HTTP Client | [owner plan](../../../app/http-client/docs/dev/issue-200/README.md) | manifest、notice、fixture corpus 与 app 侧 package smoke | `WP-2001` |
| xtask | [owner plan](../../../crates/xtask/docs/dev/issue-200/README.md) | manifest 驱动 staging/verify CLI 与单元验证 | `WP-2002` |

## 适用性

| S-ID | Surface | 状态 | 决定 |
| --- | --- | --- | --- |
| `S-01` | Workspace、文件、模块与 owner | Applicable | 分离 app、xtask 和 root release owner；不改变现有媒体运行时代码。 |
| `S-02` | GPUI 组件、布局与交互 | No change | 已有 viewer UI 只作为发行验证对象。 |
| `S-03` | Entity、Store、Global、identity 与 projections | No change | 维持 #199 的 `ResponsePane`/media/PDF authority。 |
| `S-04` | Actions、events、subscriptions、focus 与 windows | No change | 不新增动作或窗口行为。 |
| `S-05` | 异步任务、并发、取消与 shutdown | No change | 维持 #199 的 session/task 生命周期。 |
| `S-06` | 数据获取与 Operation state | No change | 不修改 HTTP 与 preview Transition。 |
| `S-07` | Forms 与 editable state | N/A | 本轮没有表单或编辑状态。 |
| `S-08` | 跨 crate、platform 与 external contracts | Applicable | 冻结 manifest 驱动的 GStreamer runtime/plugin contract。 |
| `S-09` | Error identity、传播、恢复与 error UI | No change | 缺 runtime/plugin 继续走既有 viewer-local failure。 |
| `S-10` | 数据库、持久化与 migrations | N/A | 不涉及持久化数据。 |
| `S-11` | Generated、synchronized、copied 或 vendored content | Applicable | manifest、notices 与 audit 输出有明确 source/生成边界；runtime binary 不提交 Git。 |
| `S-12` | Icons 与 assets | Applicable | 添加受许可审计的媒体/PDF fixture；不纳入 runtime binary。 |
| `S-13` | Fluent i18n 与 bundle localization | No change | 既有 preview 文案不变。 |
| `S-14` | Security、privacy 与 credentials | Applicable | release audit/diagnostics 不记录 response 内容、URL、header 或本地敏感路径。 |
| `S-15` | Observability 与 diagnostics | Applicable | 记录安全的 runtime/plugin/fixture/package evidence。 |
| `S-16` | Packaging、platform behavior 与 CI/release | Applicable | macOS/Windows 私有 runtime、Linux package dependency、CI 与安装后 smoke 同步验证。 |
| `S-17` | Dependencies、frameworks、Git sources 与 toolchains | Applicable | 维持已有精确 Rust/fork pin；审计 native 版本、来源、hash、license 与插件清单。 |
| `S-18` | Owner documentation、indexes 与 ADRs | Applicable | 本计划与前序/owner/index 双向链接；无需 ADR。 |
| `S-19` | Validation 与 completion evidence | Applicable | Cargo、bundle、package、实际 playback/PDF pagination 分层记录。 |

## 证据与决定

| ID | 分类 | 事实或决定 | 后果 |
| --- | --- | --- | --- |
| `E-2000` | Current fact | #199 的媒体/PDF 代码和定向自动化已经存在；其历史计划记录实际基线。 | 本计划只验证、打包并收口发行工件。 |
| `E-2001` | Current fact | `app/http-client/build-assets/gstreamer/` 与 `app/http-client/test-data/response-preview/` 尚无可提交的发行 manifest/notices/fixture corpus。 | `C-2001` 尚未满足。 |
| `E-2002` | Scope decision | #199 原始验收不包含媒体/PDF；未完成发行工作由独立 #200 承接。 | #199 不再被 `C-2001` 阻塞。 |
| `D-2000` | Decision | #200 以 #199 既有实现为唯一产品基线，不复制或重做其 L/D/R/T 细节。 | 变更集中在发行工件、staging、验证和相应文档。 |
| `D-2001` | Decision | 发行清单缺失、hash/license/plugin 不匹配、fixture 无可审计来源或任一目标平台 smoke 失败时，发布 gate 必须失败。 | 不以降级打包或仅 Cargo 结果宣称完成。 |

### C-2001：三平台媒体发行契约

- **Producer：** `app/http-client` 提供 app-local runtime manifest、third-party notices 与 fixture provenance；
  `crates/xtask` 仅按该 manifest stage/verify；root scripts/CI 安装开发 SDK 并调用相应 verifier。
- **Consumers：** macOS arm64 安装包、Windows x86_64 安装包和受支持 Linux `.deb`；三者均验证同一 MVP
  element/codec contract 与对应许可证记录。
- **Compatibility：** 既有 HTTP 请求/Response 行为不变。macOS/Windows 不依赖终端用户 PATH；Linux 用明确的
  系统 package dependency 满足 ABI。
- **Failure：** artifact、plugin、license 或 codec smoke 任一缺失即为 package failure；普通 HTTP Client
  功能与 PDF 不应因 GStreamer runtime 缺失而失效。

## 工作包

### WP-2000：冻结 runtime、许可与 fixture producer

**Owner：** root hub + `app/http-client`

**Prerequisites：** `E-2000`–`E-2002`、`D-2000`–`D-2001`
**Outcome：** 每个平台的 runtime 来源、hash、文件/plugin/license whitelist，及可再分发 fixture 的来源与许可
都可审计。

1. 在 HTTP Client owner plan 所列路径完成 manifest、notices 与 fixture corpus。
2. 将 macOS/Windows 的私有 bundle closure 与 Linux 系统 dependency 转为可验证清单。
3. 让缺失、hash 不符、license/plugin 漏项的验证硬失败。

### WP-2001：完成 HTTP Client 侧发行资产与 smoke

**Owner：** `app/http-client`

**Owner plan：** [HTTP Client owner plan](../../../app/http-client/docs/dev/issue-200/README.md)
**Outcome：** app 资产可由 manifest 被 stage，受许可 fixture 覆盖 MVP codec/PDF，并在安装后运行 viewer smoke。

### WP-2002：完成 xtask staging 与 release verifier

**Owner：** `crates/xtask`

**Owner plan：** [xtask owner plan](../../../crates/xtask/docs/dev/issue-200/README.md)
**Outcome：** xtask 只消费 app-local manifest，并拒绝不完整 release artifact。

### WP-2003：CI、安装包与实际 UI 验收

**Owner：** root hub

**Prerequisites：** `WP-2000`–`WP-2002`、`C-2001`
**Outcome：** 三平台开发 SDK、CI、package inspection、安装后 codec smoke 与桌面实际音频/视频播放、PDF 翻页
均有逐平台证据。

1. CI 将开发 SDK contract 与 release artifact contract 分开；开发 SDK 成功不替代 release verifier。
2. 在 macOS/Windows/Linux 包上验证 runtime path、element/plugin、fixture codec 和 notices。
3. 在桌面 app 实际执行一组音频、一组视频与 PDF Previous/Next；记录环境、fixture 和结果。

## 验证与完成条件

| Requirement | Evidence | 完成条件 |
| --- | --- | --- |
| `C-2001` manifest/notice/fixture | manifest parser/whitelist/license/provenance tests | 无占位值，来源、hash、license、plugin 和 fixture 全部可追溯。 |
| SDK 与 staging | app/xtask focused check、verifier、CI logs | 三平台 build SDK 与 package artifact 分别通过对应 gate。 |
| Release behavior | 安装后 element/codec smoke、实际桌面 UI | 三平台 package 均可播放 MVP 音频/视频并翻页 PDF；失败记录不被自动化替代。 |
| Regression | HTTP Client focused/full tests、fmt、Clippy、diff check | 已有 HTTP/Response 行为保持通过。 |

`Done` 仅在 `WP-2000`–`WP-2003` 及 `C-2001` 全部完成、实际命令/平台/安装包证据回填，并同步 owner/root
索引后设置。若平台资源不可用，保持 `In progress` 并明确缺失平台，不能以另两平台或 Cargo 结果代替。
