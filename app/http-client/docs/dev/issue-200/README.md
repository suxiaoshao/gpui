# HTTP Client：Rodio AudioDriver 与 GStreamer app 链路删除

## Root Hub 与 owner 边界

- Plan ID：`issue-200`
- Root hub：[Issue #200](../../../../../docs/dev/issue-200/README.md)
- Owner directory：`app/http-client`
- Owner index：[HTTP Client 开发文档](../README.md)
- Root-owned IDs consumed：`S-01`–`S-19`、`C-2001`、`ERR-2001`–`ERR-2004`、`D-2000`–`D-2003`
- Owner-authored IDs：`F-2000`–`F-2008`、`L-2000`–`L-2002`、`ST-2000`、`R-2000`–`R-2002`、`T-2000`–`T-2003`
- Assigned WP：`WP-2001`
- Owns：Response audio adapter、app dependency、app-local GStreamer assets/runtime bootstrap 删除，以及其定向测试。
- Does not own：xtask/root scripts/workflows 的删除、Linux system package 安装或三平台总体验收。

[#199 媒体历史记录](../issue-199/response-media-and-pdf-preview-plan.md) 已 `Superseded`。本计划不恢复其中的
GStreamer、视频或 package-manifest 工作。

## Owner-local 证据与决定

| ID | 分类 | 事实 | 计划后果 |
| --- | --- | --- | --- |
| `E-2000` | Current fact | `ResponseReadLease::materialize_media_asset` 产生 session-owned 临时文件，`ResponseAssetLease::open` 返回独立 `File`。 | Rodio 直接消费 File，仍不暴露 collector spill path。 |
| `E-2001` | Current fact | `MediaRuntime`、`MediaDriver`、`MediaCommand`、`MediaDriverEvent` 已隔离 ResponsePane 与具体 backend。 | 保持公共的 app-private session contract。 |
| `E-2002` | Upstream fact | Rodio `Player` 的 drop 会停止声音；`try_seek` 可能阻塞约 0–5 ms。 | command 继续通过当前 driver worker，不在 GPUI render/event handler 内 seek。 |
| `D-2004` | Owner-local decision | 一个 active preview 只持有一个 `MixerDeviceSink`、一个 `Player` 和一个已 append 的 `Decoder<File>`；不可复用跨 Response 的 player。 | Response/mode 切换的旧 driver drop 即停止输出并删除临时 asset。 |
| `D-2005` | Owner-local decision | 默认为 `Paused`：先 pause Player、再 append Decoder，成功准备后才发送 `Prepared`。 | 不自动播放。 |

## 文件与工件边界

```text
app/http-client/
├── Cargo.toml                                             # F-2000 [Modify, handwritten] remove gstreamer; pin Rodio 0.22.2 features; declare Deb ALSA runtime
├── src/features/request/response/media.rs                  # F-2001 [Modify, handwritten] stop exporting/declaring GStreamer runtime
├── src/features/request/response/media/asset.rs            # F-2002 [Modify, handwritten] retain private File opening; remove URI-only GStreamer projection
├── src/features/request/response/media/audio.rs            # F-2003 [Modify, handwritten] AudioDriver over Rodio/CPAL
├── src/features/request/response/media/runtime.rs           # F-2004 [Delete, handwritten] process-global GStreamer bootstrap/environment contract
├── src/features/request/response/media/session.rs           # F-2005 [Modify, handwritten] remove GStreamer plugin/software-decoder-only state, retain generic session contract
├── src/features/request/response.rs                         # F-2006 [Modify, handwritten] keep prepare/event task ownership while removing decoder-policy retry
├── build-assets/gstreamer/runtime-manifest.toml             # F-2007 [Delete, handwritten] obsolete private runtime input
├── build-assets/gstreamer/THIRD_PARTY_NOTICES.md            # F-2008 [Delete, handwritten] obsolete runtime notice input
└── docs/dev/issue-200/README.md                             # this owner plan
```

`test-data`、runtime installers、Framework/DLL/plugins、license manifest 和 package payload 都不是 Rodio
输入，也不由此 owner 新增。PDF 文件、renderer、viewer 和 PDF tests 不在此工作包。

### 当前实施状态（2026-08-14）

Rodio/CPAL/Symphonia AudioDriver 迁移与 app-local GStreamer source/assets 删除已落地；PDF 保持原实现，视频
不恢复。此记录不代表实际设备播放、UI 操作或三平台验证已经执行。

## Owner-local contracts

### L-2000：AudioDriver target boundary

```rust
pub(crate) struct AudioDriver {
    commands: async_channel::Sender<MediaCommand>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl AudioDriver {
    pub(crate) fn prepare(asset: ResponseAssetLease) -> Result<AudioPrepared, MediaProblem>;
}

impl MediaDriver for AudioDriver {
    fn command(&mut self, command: MediaCommand) -> Result<(), MediaProblem>;
}
```

- `prepare` 仅由 `RequestView::build_audio_prepare_task` 的 background worker 调用；它先 `asset.open()`，然后
  调用 `rodio::Decoder::try_from(file)`，并从 `total_duration()` 建立 `MediaMetadata`。
- 通过 `rodio::DeviceSinkBuilder::from_default_device()` 与 `open_sink_or_fallback()` 取得 `MixerDeviceSink`，创建
  `rodio::Player::connect_new(&sink.mixer())`。Driver worker 独占 sink/Player/source/asset 生命周期。
- `MediaCommand::Play` / `Pause` / `Seek` / `SetVolume` / `SetMuted` / `PollPosition` / `Stop` 的映射固定为
  `Player::play` / `pause` / `try_seek` / `set_volume` / `set_volume(0.0)` / `get_pos` + `empty` / `stop`。
  mute 不改变 stored UI volume；unmute 恢复其值。
- `PollPosition` 产生 `Position`。`empty()` 仅产生 `Ended`，遵守根计划的 `ERR-2003` 边界。
- GStreamer plugin/rank/environment、`DecoderPolicy::SoftwareOnly` 和 software retry 均删除；不引入其他
  decoder fallback。

### L-2001：MediaProblem cleanup

`MediaProblemKind` 保留 `AssetRead`、`TemporaryAsset`、`RuntimeUnavailable`、`Decode`、`Control` 与
`Internal`。删除只为 GStreamer plugin diagnostic 服务的 `PluginMissing`/`MediaProblemDetail::Plugin`。
保留 generic locale key，删除旧 `response-media-plugin-missing` key 与变量契约。`RuntimeUnavailable` 映射
`ERR-2001`，准备期 decoder failure 映射 `ERR-2002`，seek failure 映射 `ERR-2004`。

### L-2002：明确格式范围

```toml
rodio = { version = "=0.22.2", default-features = false, features = [
  "playback", "symphonia-aac", "symphonia-flac", "symphonia-isomp4",
  "symphonia-mkv", "symphonia-mp3", "symphonia-ogg", "symphonia-pcm",
  "symphonia-vorbis", "symphonia-wav",
] }
```

该列表是唯一的 codec policy：支持范围是功能组合可解码的 AAC/FLAC/MP4/MKV/MP3/Ogg/PCM/Vorbis/WAV。
不添加 `symphonia-all`，不声明或测试 Opus；`audio/ogg; codecs=opus` 和 `audio/webm; codecs=opus` 仍会进入
Audio viewer，但 decoder prepare 必须稳定投影为 `Decode`，而用户仍可切到 Bytes/Hex/Base64 或 Save。

## ST-2000：单 preview 音频生命周期

- **Authority：** `ResponsePane::media: MediaRuntime`；具体设备/decoder 状态由 active `AudioDriver` 独占。
- **Initialization/lifetime：** preview mode 选中 Audio 后，`build_audio_prepare_task` materialize asset 并创建
  driver；`Prepared` 后由 `MediaRuntime` 处于 Paused/Playing/Ended。一个 token 只接收自己的 event。
- **Readers：** `render_media`、`sync_media_controls` 与当前 token-checked event task。
- **Mutation：** 用户 control 转为 `MediaCommand`；driver worker 是唯一调用 Rodio Player 的 owner。
- **Publication：** metadata、position、Ended 通过现有 `MediaDriverEvents` 返回；不会把 Rodio 类型带入 GPUI。
- **Reset/cancellation：** 新 Send、Clear、Response/mode switch 或 RequestView drop 先终止 event task，再 Stop/drop
  driver。drop Player/sink 停止输出，asset 随 driver 释放。

## WP-2001：实现 Rodio adapter 并删除 app GStreamer 残留

**Prerequisites:** `C-2001`、`ERR-2001`–`ERR-2004`、`D-2000`–`D-2005`。

1. 以 F-2000 的精确 feature list 更新依赖；由 Cargo 更新 lockfile，删除 `gstreamer`/`gstreamer-sys` 解析项，
   不手改 lockfile。
2. 按 L-2000 用 F-2002 的 session-private File 实现 F-2003；prepare 或 device-open failure 不产生可播放状态。
3. 删除 F-2004、F-2007、F-2008，并从 F-2001/F-2005/F-2006 消除 runtime init、plugin detail、decoder policy
   与 retry 逻辑；保留 `MediaRuntime` token/teardown/controls 的既有 owner。
4. 保留 backend-neutral locale 与 i18n required-key；删除无消费者的 plugin-specific key，PDF key 和普通 Audio UI 文案不变。
5. 添加/调整下列测试，不以真实设备播放替代 unit coverage。

| R-ID | T-ID | Scenario | Assertion |
| --- | --- | --- | --- |
| `R-2000` | `T-2000` | fake driver/session transition | Paused start、play/pause、volume/mute、seek、stop 和 token stale protection 不变。 |
| `R-2001` | `T-2001` | decoder test + fake session driver | 损坏输入 -> `Decode`；driver command failure -> terminal `Control`；默认设备失败映射由三平台实际验证覆盖。 |
| `R-2002` | `T-2002` | static residual scan / manifest test | app source、assets、locales 和 Cargo graph 无 GStreamer/plugin/rank/runtime environment surface。 |
| `R-2002` | `T-2003` | PDF and byte viewer regressions | PDF 前后翻页及 Bytes/Hex/Base64/Save 不受音频失败影响。 |

### Focused validation

| Command/scenario | Purpose | Expected evidence |
| --- | --- | --- |
| `cargo fmt --all -- --check` | format changed Rust source | 由 root final gate 记录 |
| `cargo test -p http-client --bin http-client --all-features --locked --no-fail-fast` | session/decoder/error/PDF/viewer regression | 156/156 passed |
| `cargo clippy -p http-client --all-targets --all-features --locked -- -D warnings` | strict app lint | passed |
| scoped residual scan | app source/assets/Cargo stale surface | active GStreamer/code/config references为 0；仅计划历史说明保留名称 |

`WP-2001` 的代码与自动化已完成。真实 macOS/Windows/Linux 播放留给 root `WP-2003`，不伪装为本轮
自动化证据。
