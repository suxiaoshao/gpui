# Issue #200：HTTP Client Response 音频迁移与 GStreamer 删除

## 状态与范围

- 状态：`In progress`
- 关联 issue：[#200](https://github.com/suxiaoshao/gpui/issues/200)
- Plan ID：`issue-200`
- Root hub：`docs/dev/issue-200/README.md`
- Affected owners：`app/http-client`、`crates/xtask`、root `script/` 与 `.github/workflows/`
- Release gate：`C-2001`；三个目标平台必须用其真实默认音频设备完成构建与播放验证。
- Implementation references：当前工作树已完成 Rodio/CPAL/Symphonia 与 `libasound2-dev` 的实现迁移，并删除 active
  GStreamer code/packaging 链路；实现与验证记录由 [PR #202](https://github.com/suxiaoshao/gpui/pull/202) 承载。

Issue #199 的 [媒体/PDF 历史记录](../../../app/http-client/docs/dev/issue-199/response-media-and-pdf-preview-plan.md)
已 `Superseded`。本计划是唯一的执行入口：保留现有 PDF 只读预览，移除视频，使用 Rodio 0.22.2 的
`playback` + 指定 Symphonia feature 实现音频预览，并完整删除 GStreamer 的产品、运行时、打包、脚本与 CI
链路。

### 高影响变更摘要

| 审计面 | 结果 | IDs |
| --- | --- | --- |
| Workspace/crate ownership | [Delete] `crates/xtask` 不再拥有 GStreamer staging/verifier；音频后端只在 HTTP Client 内。 | `D-2000`、`WP-2002` |
| Runtime/跨边界 | [Breaking] `MediaDriver` 的具体音频实现从 GStreamer pipeline 改为 Rodio/CPAL OS sink；HTTP 请求与 Response authority 不变。 | `C-2001`、`ERR-2001`–`ERR-2004`、`WP-2001` |
| Dependencies | [Modify] 删除 `gstreamer`，固定 `rodio = 0.22.2`；其播放后端为 CPAL，解码后端为 Symphonia。 | `D-2001`、`WP-2001` |
| Packaging/CI | [Delete] 删除私有 runtime、manifest、xtask CLI、GStreamer scripts/workflows；[Modify] Linux 构建机安装 ALSA 开发包，`.deb` 声明 `libasound2`。 | `D-2002`、`WP-2002`、`WP-2003` |
| User-visible behavior | [Breaking] 视频继续不支持；Opus response 不提供音频预览；其他未支持/损坏音频仍保留 Headers、Bytes/Hex/Base64 和 Save。 | `D-2003`、`ERR-2003` |

### 目标

让已完整接收的 `audio/*` Response 使用 OS 原生输出播放，不要求最终用户安装或随安装包携带 GStreamer；
保留已有播放、暂停、seek、音量、静音、位置、时长、结束和销毁语义。PDF 只读预览保持原状。

### 非目标

- 不改 `RequestRuntime`、transport、Response 收集、Store、History、多 tab、Repair 或 PDF 实现。
- 不重新请求或流式播放 Response body；仍只播放完整、已受现有 capture limit 约束的 Response asset。
- 不实现视频、录音、设备选择、播放列表、倍速、均衡器或媒体编辑。
- 不把 `Player::empty()` 推断为“运行期解码错误”，也不伪造 Rodio 没有公开提供的错误回调。

### 用户决定

- 使用 `rodio = 0.22.2`，完整删除 GStreamer 产品与打包链路。
- 保留 PDF；视频已删除且不属于本 issue 的验收范围。

### 兼容与迁移策略

这是一项未发布应用内的有意不兼容替换：不保留 GStreamer runtime、环境变量、manifest、包内 plugin
目录或回退分支。没有持久化数据和配置迁移。回滚只能恢复整个依赖/代码/打包链路的同一提交，不能在一个
安装包内同时携带两套后端。

## 计划地图

| Scope | 文档 | 负责范围 | IDs / WPs |
| --- | --- | --- | --- |
| Root hub | 本文 | 跨 owner 决定、CPAL/OS 契约、错误/依赖/平台边界、顺序和汇总证据 | `S-*`、`C-2001`、`ERR-*`、`D-*`、`WP-2000`、`WP-2003` |
| HTTP Client | [owner plan](../../../app/http-client/docs/dev/issue-200/README.md) | Rodio audio driver、Response asset/GPUI projection、app manifest/asset 删除和测试 | `F-2000`–`F-2008`、`L-*`、`ST-*`、`WP-2001` |
| xtask | [owner plan](../../../crates/xtask/docs/dev/issue-200/README.md) | 删除 GStreamer bundle CLI、模块及其测试/文档入口；保留通用 Deb depends 映射 | `F-2100`–`F-2108`、`WP-2002` |

## 适用性

| S-ID | Canonical surface | 状态 | 当前证据 | 目标决定 / owner |
| --- | --- | --- | --- | --- |
| `S-01` | Workspace、文件、模块与 owner | Applicable | 迁移基线包含 `media/runtime.rs` 与 xtask/root native runtime 链路；当前实现已删除这些 owner。 | 删除结果由 `WP-2001`–`WP-2003` 验证。 |
| `S-02` | GPUI 组件、布局与交互 | No change | `ResponsePane` 已提供 Audio 控制与 PDF viewer。 | 保持现有控制组成；`WP-2001` 只接入同一 `MediaRuntime` 投影。 |
| `S-03` | Entity、Store、Global、identity 与 projections | No change | Response pane 和 `PreviewToken` 已是预览 authority。 | 不引入 Store/Global。 |
| `S-04` | Actions、events、subscriptions、focus 与 windows | No change | 没有新增 action/window。 | 保留现有触发和焦点。 |
| `S-05` | Async tasks、并发、取消与 shutdown | Applicable | Audio prepare/event bridge 已是 owner-bound task。 | 每预览一个 Rodio driver；停止/Drop 释放 Player 和 device sink；`WP-2001`。 |
| `S-06` | 数据获取与 Operation state | No change | HTTP Response 已完整 materialize 后才进入 viewer。 | 不改 request state machine。 |
| `S-07` | Forms 与 editable state | N/A | 本轮没有表单。 | — |
| `S-08` | 跨 crate、platform 与 external contracts | Applicable | 音频输出从 native GStreamer 改为 Rodio/CPAL。 | `C-2001`。 |
| `S-09` | Error identity、传播、恢复与 error UI | Applicable | 现有 `MediaProblemKind` 负责 viewer-local failure。 | `ERR-2001`–`ERR-2004`。 |
| `S-10` | 数据库、持久化与 migrations | N/A | Response asset 和 preview 都是临时状态。 | — |
| `S-11` | Generated、synchronized、copied 或 vendored content | Applicable | runtime manifest/notices 是历史 package input。 | 删除全部，不新增生成物。 |
| `S-12` | Icons 与 assets | Applicable | `build-assets/gstreamer/` 是历史 bundle asset。 | 删除；PDF 资产不变。 |
| `S-13` | Fluent i18n 与 bundle localization | Applicable | generic audio/PDF keys 覆盖 `RuntimeUnavailable`/`Decode`/`Control`，旧 plugin-specific key 已无消费者。 | 保留 generic key，删除旧 plugin-specific key 及变量契约。 |
| `S-14` | Security、privacy 与 credentials | No change | Response bytes/headers/URL 不进入媒体错误文案。 | 继续只暴露分类错误。 |
| `S-15` | Observability 与 diagnostics | Applicable | GStreamer plugin diagnostics 将消失。 | Rodio/CPAL 只记录静态错误类别和平台，禁止 response 内容；`WP-2001`。 |
| `S-16` | Packaging、platform behavior 与 CI/release | Applicable | 历史 self-contained runtime 需要 xtask/scripts/workflows。 | 删除；Linux build 仅保留 ALSA development prerequisite；`WP-2002`–`WP-2003`。 |
| `S-17` | Dependencies、frameworks、Git sources 与 toolchains | Applicable | `Cargo.toml` 与 `Cargo.lock` 已解析 Rodio 0.22.2、CPAL 0.17.3、Symphonia 0.5.5。 | 依赖库存与复用决策如下；`WP-2001`。 |
| `S-18` | Owner documentation、indexes 与 ADRs | Applicable | #199 文档仍含旧实施步骤。 | #199 收缩为 Superseded record，索引只指向本计划。 |
| `S-19` | Validation 与 completion evidence | Applicable | HTTP Client 156 tests、xtask 12 tests 与两者 strict Clippy 已通过。 | 保留未执行的真实设备/UI/三平台播放边界；`WP-2003`。 |

## 证据与决定

| ID | 分类 | 事实或决定 | 证据 | 计划后果 |
| --- | --- | --- | --- | --- |
| `E-2000` | Baseline fact | 迁移前 Audio driver 直接使用 GStreamer；Response/mode/session 抽象已存在。 | 变更前的 `app/http-client/src/features/request/response/media/{audio,runtime,session}.rs` | 保留 session contract，替换 adapter。 |
| `E-2001` | Upstream fact | Rodio 0.22.2 提供 `DeviceSinkBuilder`、`Player`、`Decoder`；`Player` 支持 pause/play/seek/volume/position/empty，drop 会停止声音。 | [Rodio docs](https://docs.rs/rodio/0.22.2/rodio/)、[Player](https://docs.rs/rodio/0.22.2/rodio/struct.Player.html) | 可覆盖当前只读单轨控制。 |
| `E-2002` | Upstream fact | Rodio 的 `playback` feature 依赖 CPAL；当前指定 codec features 解析为 Symphonia 0.5.5。 | [Rodio Cargo metadata](https://docs.rs/crate/rodio/0.22.2/source/Cargo.toml) | 无 GStreamer runtime；需要 CPAL 平台前提。 |
| `E-2003` | Upstream fact | Rodio 0.22.2 没有 `symphonia-opus` feature；当前 feature set 不能解码 Opus。 | 同上，features list | `audio/ogg; codecs=opus` 与 `audio/webm; codecs=opus` 是明确不支持边界。 |
| `E-2004` | Current fact | Linux bootstrap 使用 `script/install-linux.sh` 统一安装构建依赖。 | `script/bootstrap`、`script/install-linux.sh` | 在该入口添加 ALSA development package。 |
| `E-2005` | User decision | Rodio 0.22.2 替换 GStreamer，GStreamer 链路完整删除；PDF 保留、视频删除。 | 本 issue 对话 | `D-2000`–`D-2003`。 |

### 依赖库存与复用审计

| Dependency | Scope | Current -> target | 平台/许可事实 | 决定 |
| --- | --- | --- | --- | --- |
| `gstreamer` | HTTP Client direct runtime | `0.25.2` -> 删除 | 私有 runtime、plugins、SDK 和 staging 全部随其删除。 | `D-2000` Reuse directly: 不保留本地 wrapper。 |
| `rodio` | HTTP Client direct runtime | 无 -> `=0.22.2` | MIT OR Apache-2.0；Player 通过 CPAL 输出。 | `D-2001` Adapt: 仅以 `AudioDriver` 适配现有 session。 |
| `cpal` | Rodio transitive | 无 direct dependency -> Rodio `playback` 的 transitive | macOS CoreAudio、Windows WASAPI、Linux ALSA；Linux 构建需 ALSA development headers。 | `D-2002` Reuse directly，不写自定义 platform sink。 |
| `symphonia` | Rodio transitive decoder | GStreamer codec plugins -> `0.5.5` feature selected by Rodio | MPL-2.0；当前特性集含 AAC/FLAC/MP4/MKV/MP3/Ogg/PCM/Vorbis/WAV，不含 Opus。 | `D-2003` Reuse directly；不启用 `symphonia-all`。 |

| D-ID | 决定 | 证据 | 排除方案 | 后果 |
| --- | --- | --- | --- | --- |
| `D-2000` | 完整删除 GStreamer source、runtime、manifest、staging、scripts 与 CI。 | `E-2000`、`E-2005` | 保留 bundled runtime。 | 安装包不再有 GStreamer 私有 payload。 |
| `D-2001` | 固定 `rodio = 0.22.2`，默认 OS device 上每个 active preview 只建一个 `MixerDeviceSink` 和一个 `Player`。 | `E-2001`、用户决定 | Kira、直接 CPAL mixer、自写 decoder。 | 保持 `MediaDriver` 的单 preview 生命周期。 |
| `D-2002` | 通过 Rodio `playback` 使用 CPAL；Linux 构建安装 `libasound2-dev`，HTTP Client `.deb` 声明系统 `libasound2` runtime。 | `E-2002`、`E-2004` | 下载/携带 ALSA 或 GStreamer runtime。 | 取消所有 native runtime bundle。 |
| `D-2003` | MVP 仅启用显式 Rodio features：AAC/FLAC/MP4/MKV/MP3/Ogg/PCM/Vorbis/WAV；Opus 不支持。 | `E-2003` | 启用未验证的 all codecs 或引入第二 decoder。 | Opus preview 显示 viewer-local Decode failure，替代模式仍可用。 |

## 跨 owner 契约

### C-2001：Response asset 到 OS 音频输出

```text
ResponsePane (PreviewToken, retained task)
  -> ResponseAssetLease -> File
  -> rodio::Decoder<File> -> rodio::Player
  -> rodio::MixerDeviceSink (CPAL)
  -> macOS CoreAudio | Windows WASAPI | Linux ALSA
```

- **Producer:** `app/http-client::media::audio::AudioDriver`；它持有 device sink、Player 和已 append source 的
  生命周期。
- **Consumer:** 现有 `MediaRuntime`/`ResponsePane` 只消费 `MediaCommand` 和 `MediaDriverEvent`，不依赖 Rodio
  或 CPAL 类型。
- **Start:** background prepare 从 `ResponseAssetLease` 打开 `File`，`Decoder::try_from(file)` 取得可选时长，
  先 `Player::pause()` 再 `append(decoder)`，完成后才发布 `Paused`。
- **Control:** worker 以 `play`、`pause`、`try_seek`、`set_volume`、`get_pos`、`empty` 和 `stop` 实现既有
  command；mute 保存所选音量且把实际 Player 音量设为 `0.0`。
- **Shutdown:** Send/Clear/mode switch/window drop 先停止 event bridge，再 `stop`/drop Player 和 device sink；
  不 `detach`，不在 UI thread 执行 `try_seek`（其上游文档说明可阻塞约 0–5 ms）。
- **Compatibility:** HTTP 请求/Response 状态和 PDF viewer 不变；没有 GStreamer environment/config 兼容层。

### 错误契约

| ERR-ID | Trigger | UI/recovery | Diagnostics |
| --- | --- | --- | --- |
| `ERR-2001` | `DeviceSinkBuilder::from_default_device` 或 `open_sink_or_fallback` 无法建立 CPAL stream。 | `RuntimeUnavailable`；保留 Response 和替代 viewer，可重新选择 Audio。 | 仅类别与 target OS。 |
| `ERR-2002` | `File`/`Decoder::try_from` 在准备期间失败。 | `Decode`；不影响已完成 HTTP Response。 | 不记录路径、URL、headers 或 body。 |
| `ERR-2003` | 已构造的 Player 在播放时 `empty()`。Rodio 的公开 Player API 无法区分正常 EOF 与异步 decode failure。 | 发 `Ended`，不伪造 `Decode`；Opus/不支持格式应在 prepare 时失败并走 `ERR-2002`。 | 无额外 native error 文本。 |
| `ERR-2004` | `try_seek` 返回 `SeekError`、命令队列关闭或无效音量。 | `Control`；保留当前播放状态和其他 Response viewer。 | 仅稳定问题类别。 |

## 工作包与顺序

1. `WP-2001` 替换 app audio adapter，同时保留 PDF 和现有 session/UI contract。
2. `WP-2002` 删除 xtask 与 app-local GStreamer package assets。
3. `WP-2003` 删除 root scripts/workflows 链路，添加 ALSA prerequisite，并执行真实三平台验证。

### WP-2001：HTTP Client Rodio 音频 driver

**Owner:** `app/http-client`；详见 [owner plan](../../../app/http-client/docs/dev/issue-200/README.md)。

**Outcome:** Audio Response 可通过 Rodio/CPAL 实现现有单实例 control/EOS/lifecycle；无 GStreamer import 或
runtime bootstrap。

### WP-2002：删除 GStreamer 打包和 xtask 链路

**Owner:** `crates/xtask`；详见 [xtask owner plan](../../../crates/xtask/docs/dev/issue-200/README.md)。

**Outcome:** 没有 xtask manifest、bundle command、verifier 或 GStreamer-specific test 仍可达；app-local asset
删除属于 `WP-2001`。

### WP-2003：Linux prerequisite、CI 与平台验证

**Owner:** root `script/` 与 `.github/workflows/`。

1. 从 `script/bootstrap`/`script/install-linux.sh` 的统一入口安装 `libasound2-dev`，并由 app bundle metadata 声明
   `.deb` 的 `libasound2` 运行依赖；不安装 GStreamer SDK/runtime。
2. 删除 `script/audit-gstreamer-windows.ps1`、`script/build-gstreamer-linux-runtime.sh`、
   `script/install-gstreamer-macos.sh`、`script/install-gstreamer-windows.ps1`、
   `script/prepare-gstreamer-macos-runtime.sh`、`script/prepare-gstreamer-macos-sdk.sh`。
3. 删除 `.github/workflows/gstreamer-runtime-release.yml` 和
   `.github/workflows/gstreamer-windows-audit.yml`，并从 `.github/workflows/ci.yml` 清除 GStreamer setup；CI
   继续经 `script/bootstrap` 获得 Linux ALSA prerequisite。
4. 完成 macOS、Windows、Linux 的 `cargo build/test`，并在每个平台真实默认设备上验证一次
   Play/Pause/Seek/Volume/Mute/EOS；PDF Previous/Next 作为回归检查。

### 当前实施状态（2026-08-14）

- `WP-2001` 的 Rodio/CPAL/Symphonia audio migration 已落地；PDF 保留，视频不恢复。
- `WP-2002` 的 active GStreamer packaging/xtask 链路已删除。
- `WP-2003` 的 Linux `libasound2-dev` prerequisite 已落地；GStreamer scripts/workflows 已删除。
- 实际桌面 UI 与 macOS/Windows/Linux 三平台播放尚未运行，故 `C-2001` 和本计划保持 `In progress`。

## 验证与完成条件

| Requirement | Owner/WP | 证据 | 通过条件 |
| --- | --- | --- | --- |
| `R-2000` 无 GStreamer 残留 | `WP-2001`–`WP-2003` | `rg -i gstreamer` 对 source/Cargo/scripts/workflows/build-assets/xtask 的残留扫描 | 只允许本计划与 #199 历史文档的引用。 |
| `R-2001` 控制/lifecycle | `WP-2001` | decoder/gain unit tests + existing fake-driver media session tests | decoder duration/redaction、mute volume、paused state、play/pause/seek/volume/mute/stop/token gate 可观察；Rodio worker 的真实 EOS/replay 留给设备验证。 |
| `R-2002` 失败边界 | `WP-2001` | 无设备、损坏输入、Opus、seek failure fixture/test double | 对应 `ERR-2001`–`ERR-2004`；不会改变 Response/Save。 |
| `R-2003` Linux build | `WP-2003` | Linux CI 的 bootstrap 后 `cargo build -p http-client --locked` | 仅依赖 ALSA development package。 |
| `R-2004` 发行回归 | `WP-2003` | macOS/Windows/Linux 默认设备人工播放；PDF 翻页 | 实际环境、命令和结果逐平台记录。 |

## 完成证据

| Evidence | Actual result |
| --- | --- |
| Implementation commits / PR | [PR #202](https://github.com/suxiaoshao/gpui/pull/202)；提交历史以该 PR 为准。 |
| 实际增改删文件与 Cargo.lock | 已落地：Rodio/CPAL/Symphonia 与 Linux ALSA prerequisite；active GStreamer code/packaging 链路已删除。 |
| Focused tests、fmt、Clippy、residual scan | `cargo test -p http-client --bin http-client --all-features --locked --no-fail-fast`：156/156；`cargo test -p xtask --all-features --locked --no-fail-fast`：12/12；两 package strict Clippy、`cargo fmt --all -- --check`、Shell/YAML 检查通过；active source/Cargo/scripts/workflows/assets 残留扫描为 0。 |
| macOS / Windows / Linux build-test | 当前 macOS 本机构建与测试通过；Windows/Linux 留给 CI。 |
| 每平台默认设备播放与 PDF 手工回归 | 未执行。 |
| 已接受的偏差与未验证边界 | 未验证边界：实际 UI、三平台播放，以及真实 Rodio worker 的设备 callback/EOS/replay 路径；自动化使用 decoder unit 与 fake-driver session 覆盖可隔离逻辑。 |

## 上游资料

- [Rodio 0.22.2 crate docs](https://docs.rs/rodio/0.22.2/rodio/)
- [Rodio 0.22.2 Player API](https://docs.rs/rodio/0.22.2/rodio/struct.Player.html)
- [Rodio 0.22.2 Cargo features/dependencies](https://docs.rs/crate/rodio/0.22.2/source/Cargo.toml)
- [CPAL platform support](https://docs.rs/cpal/latest/cpal/)
