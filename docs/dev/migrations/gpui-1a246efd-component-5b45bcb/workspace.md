# GPUI `1a246efd` / gpui-component `5b45bcb` workspace 迁移门

## 1. 状态与职责

- 迁移 ID：`gpui-1a246efd-component-5b45bcb`。
- 总计划：[README.md](README.md)。
- 工作包：`ROOT-00`、`ROOT-80`。
- 本文只负责 workspace dependency identity、支持基线、跨包验证和三平台发布门。
- package exact files/API/测试由总计划索引的 package 子计划负责。

### 当前 target 自动验证记录（2026-07-21）

- `cargo build --workspace`：通过。
- `cargo test --workspace`：通过。
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`：通过。
- `cargo tree --locked -i gpui@0.2.2`、`cargo tree --locked -i gpui-component@0.5.2`：
  均只包含计划锁定的单一 source。
- 实际 UI/Computer Use 与打包按用户约定跳过；三平台 CI 未在本轮执行。
- 这些自动证据只完成当前 target 可执行部分，不解除 `UPSTREAM-TEXT-15` 对最终发布门的阻断。

## 2. ROOT-00：固定 workspace 基线

**Prerequisites**

- 实现基线 `6351898` 存在。
- GPUI target 为 `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`。
- gpui-component target 为 `5b45bcb26b9343d91a123a4d5ed8a654360512e5`。

**Files**

- 修改根 `README.md`：Rust prerequisite 从 `1.92+` 更新为 `1.95+`。
- 修改根 `AGENTS.md`：推荐 Rust 同步为 `1.95+`。
- `Cargo.toml` / `Cargo.lock` 仅验证；target SHA 未变化时不改写。

**Dependency contract**

- direct/transitive `gpui`、`gpui_macros`、`gpui_platform` 只能有一个 Zed source SHA。
- gpui-component、assets、macros 只能有一个组件库 SHA。
- root 与 gpui-component 使用同一个 canonical Zed Git URL，由 lockfile 固定 SHA。
- tree-sitter 语言 features 只在实际使用 package 出现。

**Implementation flow**

1. 记录 `Cargo.lock` 中两组完整 SHA 和 `cargo tree` 输出。
2. 更新支持基线文字，明确它是本仓 support baseline，不是 upstream MSRV 声明。
3. 检查四个 app 的 feature tree；若出现非预期 parser，先定位 feature owner，不通过关闭全部 feature 绕过。
4. 若 target SHA 在执行前变化，停止本批次并创建新的 migration ID，而不是原地改本文。

**Errors and lifecycle**

- 若出现两套 GPUI source，不写类型转换或 compatibility wrapper；统一 source 后重建 lock。
- No runtime lifecycle change。

**No change**

- database schema/query、network/auth/proxy/cache/retry、persistence、icons、Fluent 和 bundle i18n 不变。

**Validation**

```bash
cargo tree --locked -i gpui@0.2.2
cargo tree --locked -d
cargo tree --locked -e features -p jaco
cargo tree --locked -e features -p feiwen
cargo tree --locked -e features -p http-client
cargo tree --locked -e features -p novel-download
```

**Done condition**

- repo docs 为 Rust 1.95+；Cargo graph 无重复 GPUI source；manifest/lock 无无关改动。

## 3. ROOT-80：跨 package 发布门

**Current status**

- [阻断] 本批次固定的 gpui-component `5b45bcb` 不满足 TextView current-theme/cache 契约，
  因此本文件中的 `ROOT-80` 不可在当前 target 上标为完成。
- 下列发布门是后继迁移必须继承的完整 contract；得到包含 `UPSTREAM-TEXT-15` 的新 SHA 后，
  应复制到新 hash-specific `workspace.md` 并在那里执行、记录证据，不能回写当前迁移身份。

**Prerequisites**

- 总计划登记的 THEME、FORM、JACO、FEIWEN、HTTP、NOVEL、SKILL 与 `UPSTREAM-TEXT-15`
  工作包均完成。
- workspace 已使用包含 TextView 修复的新 gpui-component SHA，并已建立后继 hash-specific
  迁移批次；这些条件满足时，实际执行 owner 已是后继 `workspace.md`，不是当前文件。

**Implementation flow**

1. 运行格式、workspace build/test/clippy、dependency graph 和 residual gates。
2. 汇总每份 package 子计划的定向自动测试证据。
3. 执行 Jaco、Feiwen、HTTP Client、Novel Download 视觉/交互 smoke。
4. 运行 macOS、Linux、Windows CI；Linux Wayland/X11 与 Windows 至少完成启动 smoke。
5. 把最终提交、PR、CI 和人工结果回填后继 hash-specific 总计划；当前总计划只补 successor
   链接并保留 blocked 历史，不回写为新 SHA 的完成状态。

**Automatic validation**

```bash
cargo fmt --all -- --check
git diff --check
cargo build --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo tree --locked -i gpui@0.2.2
cargo tree --locked -i gpui-component@0.5.2
cargo tree --locked -d
rg -n '\.bg\(cx\.theme\(\)\.' app -g '*.rs' | rg -v '\.tokens\.'
rg -n 'tokens\.[a-zA-Z0-9_]+\.opacity\(' app crates -g '*.rs'
```

第一个 residual scan 只允许 Jaco composer 中两处 gpui-component `input_background()` 计算结果；
它们是组件提供的最终 `Hsla` 输入外观，不是可替换的 `ThemeToken`。第二个 scan 预期为空，token
opacity 必须经过 `.background`。

**Manual/Computer Use matrix**

- Jaco：Material light/dark、Aurora、main/settings/about/temporary/screenshot、theme grid、
  conversation Markdown/code block 与 composer/editor、provider/MCP/prompt/shortcut dialogs；
  既有 Markdown 必须在不修改 source/revision 的情况下消费当前共享 syntax palette，且
  code-block surface 可读；静态 palette 与双 surface 关系由 `THEME-10` 自动门负责，运行时
  current-theme/cache 由 upstream test 负责。连续切换 project/model/effort/approval，
  验证 Up/Down/Enter/Escape、mouse selection、search/composer focus，不得出现 entity re-entry。
- Feiwen：titlebar route/actions、traffic lights、drag/double-click/right-click/controls、progress 0/mid/100、advanced query scroll。
- HTTP Client：URL input、request params/body、main scroll/layout。
- Novel Download：workspace background/input/result layout。
- platform：macOS/Linux/Windows CI；Linux Wayland/X11 与 Windows 启动及 Feiwen controls smoke。

**Errors and lifecycle**

- failure 返回对应 package 工作包修复；ROOT-80 不加入 fallback。
- 非目标上游能力若暴露独立问题，记录新 issue，不扩大本迁移。
- Computer Use 无法覆盖的平台由 CI artifact/manual smoke 补证据。

**Done condition**

- 所有自动门、视觉矩阵和三平台 CI 通过；无重复 source、gradient 丢失、TextView 旧主题
  cache、List 重入、焦点、titlebar、scroll 或 Taffy blocker。
