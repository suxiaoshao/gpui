# app-theme：gpui-component `5b45bcb` 主题映射迁移

## 1. 状态与范围

- 迁移 ID：`gpui-1a246efd-component-5b45bcb`。
- 总计划：[GPUI / gpui-component 迁移总计划](../../../../../docs/dev/migrations/gpui-1a246efd-component-5b45bcb/README.md)。
- GPUI source：`1d217ee39d381ac101b7cf49d3d22451ac1093fe` ->
  `1a246efd7e1b83ab568ec5e3e6c1a43a42e1abba`。
- gpui-component source：`c36b0c6ae6d14c33473f6610a27c3abc584afdf9` ->
  `5b45bcb26b9343d91a123a4d5ed8a654360512e5`。
- 工作包：`THEME-10`。
- 当前状态：Material 3 颜色投影、按钮 role/state layer、共享代码内容 palette 与 editor chrome
  已完成；除明确的按钮映射外，其他 light/dark 颜色由 baseline hash 锁定，组件几何继续使用
  gpui-component 默认值。`cargo test -p app-theme` 的 23 个测试及 workspace 自动门通过。

本计划负责共享主题层 `crates/app-theme` 的完整 Material You 投影：从
`MaterializedScheme` 生成 `ThemeConfigColors` 与 `HighlightThemeStyle`，组装唯一的
`ThemeConfig`，再由 gpui-component 安装为 `ActiveTheme`。JSON preset themes、应用背景绘制与
设置预览属于 Jaco/其他 app 子计划；应用只能消费共享主题，不生成、缓存或同步第二套代码配色。

### 目标

1. 保留 `material-color-utils` 的 Material 3 `TonalSpot` 生成逻辑，以 role、state layer 和
   component color projection 建立单向数据流。
2. 以 AndroidX Compose Material 3 的组件颜色与 state layer 为官方参考；只迁移颜色语义，
   不把 Android 组件的几何、间距或 elevation 实现搬进桌面应用。
3. 明确 gpui-component button token 的 Material 3 颜色对应关系，并保持其他既有颜色输出不变。
4. 让编辑器与 rendered Markdown 共用 `plain`、`muted` 和 syntax 代码内容色，同时保留各自独立的场景背景。
5. 让 colors 与 highlight 从同一 Material 语义模型一次性投影，删除反序列化后的重复赋值。
6. 删除“所有 `Option` 都机械填满”的错误设计目标，改用基线兼容和语义关系测试。
7. 保持 Material You 生成主题为纯色；gradient 由 JSON preset parser 提供。

### 非目标与 No change

- 不改变 material seed、theme ID、序列化格式或持久化路径。
- 不改变 JSON theme assets、Jaco preview 或 app rendering。
- 不强制编辑器背景与 Markdown 代码块背景相同；共享的是代码内容色，不是场景 chrome。
- Material 3 只作为颜色 role 与 state layer 的参考；button 的边框、圆角、padding、尺寸、
  typography、shadow/elevation 几何、动效和 focus ring 均继续由 gpui-component 负责。
- `app-theme` 不生成或覆盖 shadow；`ThemeConfig` 不再用 Material 主题统一覆盖
  gpui-component 的 `radius` / `radius_lg` 默认值。
- 不根据新增 schema 字段重新设计 surface、border、selection、overlay、skeleton 或 scrollbar
  的既有颜色；这些字段只做当前值的结构化投影和兼容性验证。
- 不在 Jaco、Feiwen 或其他应用中新增 palette、主题同步器或局部颜色 override。
- 不修复 `TextView::CodeBlock` 的主题读取时机或 styles cache；这是 gpui-component 的
  `UPSTREAM-TEXT-15`。本 crate 只保证安装进 `ActiveTheme` 的静态 palette 与 surface 语义正确。
- 不改变数据库、network/auth/cache/retry、icons、Fluent/i18n。
- 不新增依赖；继续使用当前 `material-color-utils` 与 gpui-component target source。
- gpui-component 当前只有一个 `switch_thumb`，本次不新增本地 checked/unchecked 双 token schema。

## 2. 当前证据

- `crates/app-theme/src/lib.rs` 当前直接把 Material scheme 机械铺到
  `ThemeConfigColors` 的新增字段。
- `MaterialSemanticRoles` 的 `container` / `on_container` 只在测试配置下存在，runtime
  semantic button 因此无法从正确 role pair 投影。
- `MaterialInteractiveRole` 只表达 hover/active，丢失组件 base 与 foreground 的语义关系。
- `crates/app-theme/src/tests.rs` 的完整性测试要求所有公开 `Option` 都为 `Some`，它只能证明字段被填充，不能证明 role 映射正确。
- `apply_material_highlight_tokens` 已先把 editor 字段写入 JSON map，`serde_json::from_value`
  后又逐字段写入相同颜色；这是两个投影入口，同一主题数据存在重复来源。
- 当前代码把 `editor_gutter_background` 强制设为 `editor_background`。目标 gpui-component
  仅在 gutter 为 `Some` 时使用独立颜色，否则自然继承 editor 背景；同色场景应保持 `None`。
- 当前 `MaterialSurfaceTokens` 仍分别生成 `foreground` 与 `muted_foreground`，而 highlight
  生成函数又直接从 `MaterializedScheme` 读取相同 role；若只增加新类型而不删除这两个旧入口，
  colors 与 highlight 仍可能分叉。
- gpui-component target 将 button variants 拆为 base/hover/active/foreground，并通过
  `ThemeToken` 同时表达代表色和 renderable background。
- 目标 gpui-component 的 Input editor 与 `TextView::CodeBlock` 使用同一个
  `ActiveTheme.highlight_theme.syntax` schema；前者在 render 时读取当前 theme，后者在
  `5b45bcb` 中保存 parse-time theme。共享层负责 palette 一致性，运行时生命周期由
  `UPSTREAM-TEXT-15` 修复。Input 消费 editor chrome，CodeBlock 使用 `Theme.tokens.muted` 作为表面。
- 目标版本尚未在 renderer 中读取 `editor_foreground`、`editor_line_number` 与
  `editor_active_line_number`：普通代码文本和行号分别沿用 `Theme.foreground`、
  `Theme.muted_foreground`。因此共享层必须保持 colors 与 highlight 的 plain/muted 语义一致，
  不能只填写 editor 字段。
- 颜色与交互态的官方参考固定为 AndroidX Compose Material 3 仓库提交
  `a96148d3d01d4dc8586e7adc8e04c89e5ba9fd57`：`ColorScheme.kt`、`Button.kt`、
  `FilledButtonTokens.kt`、`FilledTonalButtonTokens.kt`、`ElevatedButtonTokens.kt`、
  `OutlinedButtonTokens.kt`、`TextButtonTokens.kt` 与 `StateTokens.kt`。不再以 Material UI
  的 Material 2 组件颜色作为实现依据。
- 官方 Material 3 state layer 为 hover 8%、focus 10%、pressed 10%、dragged 16%。目标
  gpui-component 主题只暴露 button hover/active 背景，因此本 crate 只投影 hover 8% 与
  pressed 10%；focus、disabled、outline、ghost、link 和 text button 行为继续由组件实现。

## 3. 已冻结设计

唯一数据流：

```text
MaterializedScheme
  ├──> MaterialRoleSet + MaterialButtonStateLayers
  │      └──> MaterialComponentTokens ──> ThemeConfigColors
  └──> MaterialCodePalette
         ├──> ThemeConfigColors.foreground / muted_foreground
         ├──> HighlightThemeStyle.syntax
         └──> MaterialEditorChrome ─────> HighlightThemeStyle.editor

ThemeConfig { colors, highlight }
  -> Theme::apply_config
  -> ActiveTheme
       ├── Input editor: editor chrome + shared syntax
       └── TextView CodeBlock: muted surface + shared syntax
```

`ThemeConfigColors` 与 `HighlightThemeStyle` 都只是 gpui-component schema output，不承担
Material domain model。所有 component state、代码内容色和 editor chrome 都先在明确的
Material 类型中确定；最终投影只做字段映射，不重新选择颜色。应用侧没有反向同步，也没有第二份 palette。

### 核心类型

```rust,ignore
#[derive(Clone, Copy)]
struct MaterialColorPair {
    color: Argb,
    on_color: Argb,
}

#[derive(Clone, Copy)]
struct MaterialSurfaceRoles {
    surface: Argb,
    surface_container_lowest: Argb,
    surface_container_low: Argb,
    surface_container: Argb,
    surface_container_high: Argb,
    on_surface: Argb,
    on_surface_variant: Argb,
    outline: Argb,
    outline_variant: Argb,
}

#[derive(Clone, Copy)]
struct MaterialRoleSet {
    primary: MaterialColorPair,
    tonal_secondary: MaterialColorPair,
    error: MaterialColorPair,
    info: MaterialColorPair,
    success: MaterialColorPair,
    warning: MaterialColorPair,
    surfaces: MaterialSurfaceRoles,
}

#[derive(Clone, Copy)]
struct MaterialButtonStateLayers {
    hover_alpha: u8,       // Material 3 hover: 8%
    pressed_alpha: u8,     // Material 3 pressed: 10%
}

struct MaterialButtonTokens {
    background: SharedString,
    hover: SharedString,
    active: SharedString,
    foreground: SharedString,
}

struct MaterialComponentTokens {
    surface: MaterialSurfaceTokens,
    control: MaterialControlTokens,
    interaction: MaterialInteractionTokens,
    status: MaterialStatusTokens,
    overlay: SharedString,
    window_border: SharedString,
}

#[derive(Clone, Copy)]
struct MaterialSyntaxPalette {
    keyword: Argb,
    function: Argb,
    type_: Argb,
    property: Argb,
    attribute_link: Argb,
    tag: Argb,
    string: Argb,
    constant: Argb,
}

#[derive(Clone, Copy)]
struct MaterialCodePalette {
    plain_text: Argb,
    muted_text: Argb,
    syntax: MaterialSyntaxPalette,
}

#[derive(Clone, Copy)]
struct MaterialEditorChrome {
    background: Argb,
    active_line_background: Argb,
    line_number: Argb,
    active_line_number: Argb,
    invisible: Argb,
    invisible_alpha: u8,
    gutter_background: Option<Argb>,
}

fn material_button_tokens(
    background: Argb,
    foreground: Argb,
    states: MaterialButtonStateLayers,
) -> MaterialButtonTokens;

impl MaterialCodePalette {
    fn new(scheme: &MaterializedScheme, surfaces: &MaterialSurfaceRoles) -> Self;
}

impl MaterialEditorChrome {
    fn new(surfaces: &MaterialSurfaceRoles, code: &MaterialCodePalette) -> Self;
}

fn build_material_theme_colors(
    roles: &MaterialRoleSet,
    states: MaterialButtonStateLayers,
    components: &MaterialComponentTokens,
    code: &MaterialCodePalette,
) -> ThemeConfigColors;

fn material_highlight_theme_style(
    roles: &MaterialRoleSet,
    states: MaterialButtonStateLayers,
    code: &MaterialCodePalette,
    editor: &MaterialEditorChrome,
) -> HighlightThemeStyle;
```

- 类型保持 crate-private；不扩大 `app-theme` public API。
- `MaterialRoleSet` 只保存最终会被投影的 base/foreground pairs 与 neutral surfaces；当前
  `MaterialSemanticRoles` 由两字段 `MaterialColorPair` 取代，不为未使用的 container role
  扩大 runtime model。
- 当前 `MaterialThemeColors` 重命名为 `MaterialComponentTokens`，明确它仍是 Material
  component projection，不是最终 gpui-component schema。
- 删除 `MaterialInteractiveRole`；button state 统一由 `material_button_tokens` 生成。
- mode 只选择对应的 dynamic scheme；button hover/pressed alpha 固定为 8%/10%。既有
  selection、overlay、skeleton、scrollbar 等非 button 颜色继续由当前投影函数负责，不能借本次
  重构改用另一套 state/elevation 规则。
- `MaterialCodePalette::new` 只为 syntax 使用 `MaterializedScheme` 的 seed、harmonize 与 dynamic
  tone；`plain_text` / `muted_text` 必须分别从 `surfaces.on_surface` /
  `surfaces.on_surface_variant` 读取，不能再次直接读取 scheme。
- `MaterialEditorChrome::new` 只读取 `MaterialSurfaceRoles` 与 `MaterialCodePalette`；neutral
  editor background 直接使用 surface role，不经过 syntax seed 生成器。
- `MaterialSurfaceTokens` 删除 `foreground` / `muted_foreground`；
  `build_material_theme_colors` 必须从同一个 `MaterialCodePalette` 写入最终 colors 字段。
- `material_highlight_theme_style` 通过一个 JSON map 投影 status/editor/syntax，并且只调用一次
  `serde_json::from_value`；反序列化成功后直接返回，不再修改字段。

### 固定映射与兼容边界

Button color 以官方 Material 3 role 为依据，但必须适配 gpui-component 已有 variant，而不是
改变组件几何。映射固定为：

| gpui-component token family | Material 3 color role | State background |
| --- | --- | --- |
| default button | 保留当前 filled-tonal 语义：`secondary_container / on_secondary_container` | `on_secondary_container` 以 8%/10% 叠加到 base |
| primary button | Filled：`primary / on_primary` | `on_primary` 以 8%/10% 叠加到 base |
| secondary button | Filled Tonal：`secondary_container / on_secondary_container` | `on_secondary_container` 以 8%/10% 叠加到 base |
| danger button | Filled destructive extension：`error / on_error` | `on_error` 以 8%/10% 叠加到 base |
| info/success/warning button | 本仓 M3 HCT 扩展 role 的 `color / on_color` | 各自 `on_color` 以 8%/10% 叠加到 base |

- default variant 不改成 AndroidX `ElevatedButton` 的 `surface_container_low / primary`：
  gpui-component 的 Default 同时拥有自己的 border/shadow 契约，并非 Material 组件的一一映射；
  本次以保留已经调好的颜色为优先。
- danger/info/success/warning 不改用 `container / on_container`，避免把已经确认的 filled
  semantic button 变成新的 tonal 外观。
- outline、ghost、link、text、selected 与 disabled 状态仍使用 gpui-component 当前实现；
  `app-theme` 不为其新增平行 token 或透明度规则。
- 除上表和代码高亮共享来源外，`ThemeConfigColors` 的所有既有字段以 `6351898` 输出为 golden
  baseline。重构可以改变内部类型和生成顺序，不得改变最终 light/dark 值。

代码内容与场景表面的固定映射：

| Shared theme output | Material role / source | Consumer contract |
| --- | --- | --- |
| `code.plain_text` | `on_surface` | 同时投影到 `colors.foreground`、`editor.foreground`、`syntax.embedded`、`syntax.variable` |
| `code.muted_text` | `on_surface_variant` | 同时投影到 `colors.muted_foreground`、`editor.line_number` 及下表明确的 muted captures |
| syntax roles | 现有 syntax seeds + harmonize + dynamic tone | Input editor 与 rendered Markdown 共用同一个 `HighlightThemeStyle.syntax` |
| editor background | `surface_container_lowest` | 仅用于 Input editor chrome |
| editor active line | `surface_container_low` | 仅用于 Input editor chrome |
| editor active line number | `code.plain_text` | 与当前 renderer 的 `Theme.foreground` fallback 一致 |
| editor invisible | `code.muted_text` + `MATERIAL_EDITOR_INVISIBLE_ALPHA` | 与 muted 内容语义一致 |
| editor gutter | `None` | 继承 editor background，不触发独立 gutter 配色 |
| Markdown code-block background | `colors.muted = surface_container` | 由上游 `TextView::CodeBlock` 的 `Theme.tokens.muted` 消费 |

编辑器背景与 Markdown code-block 背景有意使用不同 surface role。统一的是 plain/muted/syntax
内容 palette，而不是两个场景的容器背景。

Syntax capture 投影保持现有视觉语义，但颜色必须先进入 `MaterialCodePalette`：

| Palette role | Syntax captures |
| --- | --- |
| `plain_text` | `embedded`、`variable` |
| `muted_text` | `comment`、`comment.doc`、`operator`、`punctuation`、`punctuation.bracket`、`punctuation.delimiter`、`punctuation.list_marker` |
| `attribute_link` | `attribute`、`link_text`、`link_uri` |
| `constant` | `boolean`、`constant`、`number`、`punctuation.special`、`string.special`、`string.special.symbol`、`text.literal` |
| `type_` | `constructor`、`type`、`variant` |
| `function` | `function`、`title`、`variable.special` |
| `keyword` | `keyword` |
| `property` | `property` |
| `string` | `string`、`string.escape`、`string.regex` |
| `tag` | `tag` |

`link_text` 继续使用 normal font style，`link_uri` 继续 italic，`title` 继续 weight 600；
本次只统一颜色来源，不改变这些 typography metadata。

## 4. 上游复用与删除

| Current local design | Target capability | Decision | Result |
| --- | --- | --- | --- |
| mechanical `ThemeConfigColors` filling | 完整 target schema | Adapt | schema 仍由上游拥有，本 crate 只做语义投影 |
| test-only semantic containers | Material dynamic scheme container roles | Replace | runtime role pair 保存完整四元组 |
| `MaterialInteractiveRole` | component base/foreground + state layers | Delete | 使用统一 helper |
| all-Option completeness test | concrete semantic invariant tests | Delete | 不再把“填满”当正确性 |
| JSON highlight projection + post-deserialize editor mutations | `HighlightThemeStyle` serde schema | Delete | 一次性投影并直接返回 |
| explicit same-color gutter override | optional upstream gutter background | Delete | 保持 `None` 并继承 editor background |
| app-side Markdown/editor palette coordination | shared `ActiveTheme.highlight_theme` | Do not add | 应用只渲染并回归，不维护第二套配色 |
| TextView current-theme/cache lifecycle | gpui-component `TextView::CodeBlock` | Upstream only | `UPSTREAM-TEXT-15` 修复；本 crate 不订阅主题或触发 reparse |
| locally generated gradient | target `ThemeToken` 可接受纯色或 gradient | Retain pure color | Material You 不人造 gradient；JSON theme owner 提供 gradient |

## 5. THEME-10 实施包

**Prerequisites**

- 总计划 `ROOT-00` 完成，dependency source 与 target SHA 固定。

**Files**

- 修改 `crates/app-theme/src/lib.rs`：role/state/component/code/editor projection。
- 修改 `crates/app-theme/src/tests.rs`：删除机械测试，增加语义 invariant tests。

**Implementation flow**

1. 从 `MaterializedScheme` 构造实际会使用的 `MaterialColorPair`；primary/error 使用官方 M3
   role，default/secondary 使用 filled-tonal role，info/success/warning 继续使用现有 HCT 扩展 role。
2. 构造固定 8%/10% 的 `MaterialButtonStateLayers`。
3. 用一个 helper 生成所有 button base/hover/active/foreground；state layer 始终使用该
   variant 的 foreground 叠加到自身 base。
4. 保留 surfaces、border、selection、overlay、skeleton、scrollbar 等当前输出，只整理其
   数据流；用 golden baseline 防止结构重构顺带调色。
5. 先构造 `MaterialRoleSet`，再以 `MaterialCodePalette::new(scheme, &roles.surfaces)` 构造
   唯一代码内容 palette，并以 `MaterialEditorChrome::new(&roles.surfaces, &code)` 构造 editor chrome；
   `colors.muted` 继续作为 Markdown code-block surface，不进入代码内容 palette。
6. 删除 `MaterialSurfaceTokens.foreground` / `muted_foreground`，由
   `build_material_theme_colors(..., &code)` 唯一写入最终 `ThemeConfigColors` 的对应字段。
7. 用 `material_highlight_theme_style` 一次性投影 status/editor/syntax，单次反序列化后直接返回。
8. `ThemeConfig` 不再设置 `radius` / `radius_lg`，由 gpui-component 默认值负责；不新增
   border、padding、size、shadow 或 focus ring 配置。
9. chart generation 继续从 role/palette 读取，不复制 component output。
10. 删除反序列化后的 editor 重复赋值、同色 gutter override、无调用者的 `material_hsla`、
   旧中间类型、机械赋值与完整性测试。

**Errors and lifecycle**

- 无 async、retry、cancellation 或 shutdown。
- 颜色 contrast/invariant 失败由测试阻止合入；运行时不 fallback 到任意颜色。
- `Theme::apply_config` 仍是最终上游投影入口；本 crate 和应用都不缓存第二份 theme palette 或 `ThemeTokens`。

**State/data ownership**

- 输入 `MaterializedScheme` 是单次生成值。
- role/state/component/code/editor projection 都是局部不可变值。
- `ThemeConfig` 是 generated Material theme 唯一的跨 crate 输出；`colors` 与 `highlight`
  是同一次生成的两个只读投影。应用只消费已安装的 `ActiveTheme`。

**Tests**

| Requirement | Test file | Proposed test name | Assertions |
| --- | --- | --- | --- |
| baseline compatibility | `src/tests.rs` | `generated_material_non_button_colors_match_baseline` | 代表 seed 的 light/dark 非 button colors 与 `6351898` golden 完全一致 |
| button role mapping | 同上 | `generated_material_buttons_follow_m3_color_roles` | default/secondary、primary、danger 与三种扩展 semantic button 使用固定 base/foreground pair |
| state layers | 同上 | `generated_material_button_states_follow_m3_state_layers` | hover/active 分别为 foreground 8%/10% 叠加到自身 base |
| component geometry ownership | 同上 | `generated_material_theme_uses_component_geometry_defaults` | `radius` / `radius_lg` 未由 generated theme 覆盖；无本地 shadow 配置 |
| shared code semantics | 同上 | `generated_material_code_content_roles_are_shared_across_theme_outputs` | foreground/editor/variable/embedded/active-line-number 共用 plain；muted foreground/editor line 与 muted capture 表逐项共用 muted；invisible 为 muted + 固定 alpha；其他 capture 按 palette role 表投影 |
| two-surface contrast | 同上 | `generated_material_code_palette_is_readable_on_editor_and_markdown_surfaces` | light/dark 的 plain、muted、`keyword/function/type/property/attribute_link/tag/string/constant` 对 editor 与 Markdown 背景的 contrast ratio 都 `>= 4.5` |
| scene separation | 同上 | `generated_material_editor_and_markdown_surfaces_remain_distinct` | editor 为 `surface_container_lowest`，Markdown muted 为 `surface_container`，两者不等 |
| gutter inheritance | 同上 | `generated_material_editor_gutter_uses_unset_inheritance` | `editor_gutter_background.is_none()` |
| existing quality | 同上 | 保留 syntax pairwise/chart/system-selection tests | 既有能力不回归；旧 editor-only readability 与 Some-completeness 测试由上述语义测试替代 |

**Validation**

```bash
cargo fmt --all -- --check
cargo test --locked -p app-theme
git diff --check
```

**Done condition**

- Material role、button state policy、component output、代码内容 palette 与 editor chrome 职责分离；
  button 颜色符合固定映射，其他主题颜色与 baseline 一致；editor 与 Markdown 共用内容色但保留不同 surface；
  component geometry、border、padding、shadow 与 focus ring 仍由 gpui-component 负责；
  `HighlightThemeStyle` 的 editor 字段使用同一个 `MaterialCodePalette` / `MaterialEditorChrome`
  投影显式写入（上游扁平 serde round-trip 会丢弃这些字段），应用中不存在第二套代码 palette/sync；
  旧机械模型和测试已删除，light/dark invariant tests 全部通过。TextView 运行时主题切换不作为
  THEME-10 的伪完成条件，必须由 `UPSTREAM-TEXT-15` 与后继 target 单独验收。

## 6. 交接审计

- [x] 所有类型、字段、helper 与 mapping 已确定。
- [x] 没有把颜色选择留给实施者。
- [x] persistence/database/network/icons/i18n 已明确 No change。
- [x] Jaco JSON themes 与 app rendering 没有混入本 crate 计划；Jaco 只承担消费与视觉回归。
