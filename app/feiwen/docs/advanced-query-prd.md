# feiwen 高级检索 PRD

## 背景

feiwen query 页已经有结构化查询模型和 DataTable 结果展示能力，但当前 UI 仍混有旧快速搜索框、旧 tags 快捷按钮区、按钮循环切换字段/条件、以及“排除”和“全部满足 / 任一满足”同级的问题。用户无法一眼判断每个条件行里的控件分别代表字段、条件和值，也无法从 PRD 直接推导出实现细节。

本次 PRD 固定第一版 UI 和实现约束：query 页只保留高级检索构建器，所有查询意图都通过结构化条件行表达。旧快速搜索能力用“标题包含 / 作者名称包含”表达；旧 tags 快捷选择能力用“标签包含全部”表达。

## 目标

- 移除旧快速搜索框、旧 tags 快捷按钮区。
- 条件行固定为四段：`字段`、`条件`、`值`、`排除`。
- 字段和条件都使用 `gpui-component::select::Select`。
- 值输入器由字段和条件唯一确定，不在实现阶段再临时决策。
- 条件组关系只包含“全部满足 / 任一满足”；排除使用独立 `Switch`。
- 排序第一版只按具体字段排序，不暴露表达式、常量或数学运算。
- PRD 必须能直接指导后续 Rust/GPUI 实现。

## 非目标

- 本阶段不实现 UI 代码，只更新 PRD 和参考图。
- 第一版不做文本 DSL、查询模板保存、导入导出、SQL 下推、migration 或全文索引。
- 第一版 UI 不暴露 `SortExpr::Add/Sub/Mul/Div/Constant`。
- 参考图只表达产品和状态，不要求后续逐像素还原。

## 页面结构

- 顶部工具条：只保留页面标题“高级检索”、查询按钮、重置按钮、抓取入口；不显示快速搜索输入框，不显示 tags 云。
- 主体布局：使用 `h_resizable("query-main")` 分为左侧查询构建器和右侧工作区。
- 右侧工作区：使用 `v_resizable("query-side")` 分为排序规则和结果表格。
- 左侧查询构建器内部滚动；排序规则内部滚动；结果区只放 DataTable，依赖 DataTable 自身滚动。
- 所有面板使用轻量边线和分隔，不使用大卡片堆叠。

## 查询模型映射

UI 状态最终转换为：

```text
QuerySpec
  filter: FilterExpr
  sorts: Vec<SortSpec>

FilterExpr
  All(Vec<FilterExpr>)
  Any(Vec<FilterExpr>)
  Not(Box<FilterExpr>)
  Predicate(Predicate)

SortSpec
  expr: SortExpr::Text | SortExpr::Number | SortExpr::Bool
  direction: Asc | Desc
```

- 条件组“全部满足”转换为 `FilterExpr::All`。
- 条件组“任一满足”转换为 `FilterExpr::Any`。
- 条件组“排除本组”Switch 开启时，在组表达式外包 `FilterExpr::Not`。
- 条件行“排除”Switch 开启时，在该行 predicate 外包 `FilterExpr::Not`。
- 空根组为 `All([])`，表示不过滤。
- 空子组按其自身关系转换：空 `All` 为 true，空 `Any` 为 false。
- 排序 UI 只生成 `SortExpr::Text/Number/Bool`，不生成数学表达式。

## 字段全集

字段 Select 按分组展示，顺序固定：

| 分组 | 字段 label | 内部字段 |
| --- | --- | --- |
| 文本字段 | 标题 | `TextField::Title` |
| 文本字段 | 简介 | `TextField::Description` |
| 文本字段 | 最新章节标题 | `TextField::LatestChapter` |
| 文本字段 | 作者名称 | `TextField::AuthorName` |
| ID 字段 | 作品 ID | `NumberField::NovelId` |
| ID 字段 | 最新章节 ID | `NumberField::LatestChapterId` |
| ID 字段 | 作者 ID | `NumberField::AuthorId` |
| 数字字段 | 字数 | `NumberField::WordCount` |
| 数字字段 | 阅读数 | `NumberField::ReadCount` |
| 数字字段 | 回复数 | `NumberField::ReplyCount` |
| 布尔字段 | 是否受限 | `BoolField::IsLimit` |
| 集合字段 | 标签 | `TagsPredicate` |
| 作者字段 | 作者 | `AuthorPredicate` |

## 字段、条件和值输入器矩阵

| 字段 | 条件 Select 选项 | 值输入器 | 转换结果 |
| --- | --- | --- | --- |
| 标题 | 包含 | `Input` placeholder `输入文本` | `Predicate::Text { Title, Contains, value }` |
| 标题 | 开头是 | `Input` | `Predicate::Text { Title, StartsWith, value }` |
| 标题 | 结尾是 | `Input` | `Predicate::Text { Title, EndsWith, value }` |
| 标题 | 等于 | `Input` | `Predicate::Text { Title, Equals, value }` |
| 简介 | 包含、开头是、结尾是、等于 | `Input` | 对应 `TextField::Description` |
| 最新章节标题 | 包含、开头是、结尾是、等于 | `Input` | 对应 `TextField::LatestChapter` |
| 作者名称 | 包含、开头是、结尾是、等于 | `Input` | 对应 `TextField::AuthorName` |
| 字数 | 等于、不等于、大于、大于等于、小于、小于等于 | `NumberInput` placeholder `输入数字` | `Predicate::Number { WordCount, NumberOp }` |
| 字数 | 介于范围 | `NumericRangeInput`，两个 `NumberInput`：`最小值`、`最大值` | `NumberOp::Between { min, max }` |
| 阅读数 | 等于、不等于、大于、大于等于、小于、小于等于 | `NumberInput` | `Predicate::Number { ReadCount, NumberOp }` |
| 阅读数 | 介于范围 | `NumericRangeInput` | `NumberOp::Between { min, max }` |
| 回复数 | 等于、不等于、大于、大于等于、小于、小于等于 | `NumberInput` | `Predicate::Number { ReplyCount, NumberOp }` |
| 回复数 | 介于范围 | `NumericRangeInput` | `NumberOp::Between { min, max }` |
| 作品 ID | 等于、不等于 | `EntityPicker` 单选，数据源为作品 ID + 标题 | `NumberOp::Eq/Ne(selected_id)` |
| 作品 ID | 大于、大于等于、小于、小于等于 | `NumberInput` placeholder `输入作品 ID` | 对应比较操作 |
| 作品 ID | 介于范围 | `NumericRangeInput`：`最小 ID`、`最大 ID` | `NumberOp::Between` |
| 最新章节 ID | 等于、不等于 | `EntityPicker` 单选，数据源为章节 ID + 最新章节标题 | `NumberOp::Eq/Ne(selected_id)` |
| 最新章节 ID | 大于、大于等于、小于、小于等于、介于范围 | `NumberInput` 或 `NumericRangeInput` | 对应比较操作 |
| 作者 ID | 等于、不等于 | `EntityPicker` 单选，数据源为作者 ID + 作者名称 | `NumberOp::Eq/Ne(selected_id)` |
| 作者 ID | 大于、大于等于、小于、小于等于、介于范围 | `NumberInput` 或 `NumericRangeInput` | 对应比较操作 |
| 是否受限 | 是否 | `Select`，选项 `是`、`否` | `Predicate::Bool { IsLimit, true/false }` |
| 标签 | 有交集 | `MultiSelectCombobox<TagOption>` | `TagsPredicate::Intersects(selected_tags)` |
| 标签 | 包含全部 | `MultiSelectCombobox<TagOption>` | `TagsPredicate::ContainsAll(selected_tags)` |
| 标签 | 被集合包含 | `MultiSelectCombobox<TagOption>` | `TagsPredicate::ContainedBy(selected_tags)` |
| 标签 | 集合相等 | `MultiSelectCombobox<TagOption>` | `TagsPredicate::Equals(selected_tags)` |
| 标签 | 为空 | 值区域显示 `无需填写` | `TagsPredicate::IsEmpty` |
| 标签 | 不为空 | 值区域显示 `无需填写` | `TagsPredicate::IsNotEmpty` |
| 作者 | 是 | `EntityPicker` 作者单选，展示作者名和作者 ID | `AuthorPredicate::Is(AuthorRef::Id/Name)` |
| 作者 | 在集合中 | `MultiSelectCombobox<AuthorOption>` | `AuthorPredicate::In(selected_authors)` |
| 作者 | 不在集合中 | `MultiSelectCombobox<AuthorOption>` | `AuthorPredicate::NotIn(selected_authors)` |

## 条件行交互

- 条件行列宽固定：字段 160px，条件 132px，值区域 flex，排除 88px，操作 32px。
- 字段为空：字段 Select 显示 `请选择字段`，条件和值禁用。
- 字段变化：清空条件和值，条件 Select 切换到该字段的合法选项。
- 条件为空：条件 Select 显示 `请选择条件`，值区域禁用并显示 `先选择条件`。
- 条件变化：清空值，并由矩阵切换 `ConditionValueEditor` variant。
- 排除 Switch：每行固定显示，label 为 `排除`；开启后该行背景使用轻微 danger tint，表达 NOT。
- 删除按钮使用 `IconName::Delete`，只有图标按钮，tooltip `删除条件`。
- 行内错误显示在值区域下方，使用 `IconName::TriangleAlert` + danger 文本。

## 条件组与嵌套

- 根组默认 `全部满足`，不可删除。
- 子组可切换 `全部满足 / 任一满足`，可删除。
- 组关系使用 `ToggleGroup::segmented()`，只包含 `全部满足` 和 `任一满足`。
- 组排除使用 `Switch::new(...).label("排除本组")`，不放进 ToggleGroup。
- 嵌套层级规则：每层左缩进 `depth * 16px`，每个非根组左侧显示 1px 竖线，组标题显示 `组 L{depth + 1}`。
- 深度 0 为根组，深度 1 为子组，深度 2 为孙组；UI 不硬限制 3 层，但参考图展示 3 层。
- 空组显示 `当前组为空，请添加条件或子组。`。
- 添加条件按钮使用 `IconName::Plus` + 文案 `添加条件`。
- 添加子组按钮使用 `IconName::Plus` + 文案 `添加子组`。
- 折叠/展开图标使用 `IconName::ChevronDown/ChevronRight`；第一版可以不实现折叠，但 PRD 和图中保留位置。

## 排序规则

第一版排序不使用表达式，只选择具体字段和方向。

| 排序字段 label | SortExpr |
| --- | --- |
| 标题 | `SortExpr::Text(TextField::Title)` |
| 作者名称 | `SortExpr::Text(TextField::AuthorName)` |
| 作品 ID | `SortExpr::Number(NumberField::NovelId)` |
| 最新章节 ID | `SortExpr::Number(NumberField::LatestChapterId)` |
| 最新章节标题 | `SortExpr::Text(TextField::LatestChapter)` |
| 字数 | `SortExpr::Number(NumberField::WordCount)` |
| 阅读数 | `SortExpr::Number(NumberField::ReadCount)` |
| 回复数 | `SortExpr::Number(NumberField::ReplyCount)` |
| 作者 ID | `SortExpr::Number(NumberField::AuthorId)` |
| 是否受限 | `SortExpr::Bool(BoolField::IsLimit)` |

- 每条排序项包含：拖拽手柄、优先级编号、排序字段 Select、方向 Select、删除。
- 方向 Select 固定选项：`升序`、`降序`。
- 添加排序默认字段为 `标题`，默认方向为 `升序`。
- 拖拽手柄使用 `IconName::EllipsisVertical`，通过 GPUI `on_drag` / `drag_over` / `on_drop` 重排排序优先级。
- 删除使用 `IconName::Delete`。
- 拖拽到自身时不改变顺序；拖拽到其他排序项时将源排序项移动到目标排序项位置。
- 排序字段未选择时显示行内错误 `请选择排序字段`。
- 多排序按列表顺序表达优先级：第 1 条优先级最高。

## 结果表格

- 使用 `gpui-component::table::DataTable` 展示完整结果，不做前 100 条截断。
- 列固定为：标题、作者、字数、阅读、回复、受限、最新章节、标签。
- 标题列固定左侧，宽度 240px。
- 字数、阅读、回复右对齐并允许 DataTable 表头排序。
- 标签列使用 `Tag` chip，超过可视宽度时在单元格内截断。
- 无结果时显示 `暂无查询结果`，并提示 `调整条件或排序规则后重新查询`。

## 组件清单

### 直接使用 gpui-component

- `select::{Select, SelectState, SelectItem, SearchableVec}`：字段、条件、布尔值、排序字段、排序方向。
- `input::{Input, InputState, NumberInput}`：文本、数字、范围数字。
- `switch::Switch`：条件排除、组排除。
- `button::{Button, ToggleGroup, Toggle}`：添加、删除、拖拽手柄、组关系。
- `tag::Tag`：已选标签、已选作者、结果标签。
- `table::{DataTable, TableState, TableDelegate}`：结果表格。
- `resizable::{h_resizable, v_resizable, resizable_panel}`：页面面板布局。
- `scroll::ScrollableElement`：查询构建器和排序区滚动。

### feiwen 局部新增组件

- `MultiSelectCombobox<T>`：多选、搜索过滤、下拉列表、已选 chip、chip 删除、空结果。
- `EntityPicker<T>`：单选实体选择器，支持搜索、ID/名称双行展示、选中值摘要。
- `ConditionValueEditor`：根据字段和条件选择具体值输入器。
- `NumericRangeInput`：两个 `NumberInput` 组合，统一处理 min/max。

这些组件先放在 `app/feiwen/src/features/query/` 目录下，不进入 `gpui-component`。

## 图标清单

| 用途 | IconName |
| --- | --- |
| 添加条件 / 添加组 / 添加排序 | `Plus` |
| 删除条件 / 删除组 / 删除排序 | `Delete` |
| 排序拖拽手柄 | `EllipsisVertical` |
| 删除 chip | `Close` |
| 错误提示 | `TriangleAlert` |
| 帮助说明 | `Info` |
| 已选项 | `Check` |
| 作者 / 作者选择器 | `User` |
| 作品 / 章节选择器 | `BookOpen` |
| Select 排序提示 | `ChevronsUpDown` |
| 展开 | `ChevronDown` |
| 折叠 | `ChevronRight` |

## 错误状态

- 字段为空：`请选择字段`。
- 条件为空：`请选择条件`。
- 文本值为空：`请输入文本`。
- 数字为空：`请输入数字`。
- 数字解析失败：`请输入有效数字`。
- 范围缺失：`请填写最小值和最大值`。
- 范围顺序错误：`最大值必须大于或等于最小值`。
- 多选集合为空：`请选择至少一项`。
- 作者或 ID 无法解析：`请选择有效项`。
- 排序字段为空：`请选择排序字段`。

错误必须显示在对应条件行或排序行内，不使用全局兜底错误。

## 参考图产物

- `advanced-query-reference.png`：总体布局图。
- `advanced-query-filter-states.png`：字段、条件、文本、数字、范围、布尔状态。
- `advanced-query-collection-states.png`：tags 多选、作者单选、作者多选、ID 选择器。
- `advanced-query-nesting-states.png`：根组、子组、三层嵌套、排除 Switch、空组。
- `advanced-query-sort-table-states.png`：具体字段排序、方向、拖拽优先级、DataTable。
- `advanced-query-error-states.png`：所有主要错误状态。

## 验收标准

- PRD 明确旧搜索和旧 tags 快捷入口已移除，排序第一版只支持具体字段。
- PRD 逐项列出字段、条件、值输入器和 `QuerySpec` 转换规则。
- PRD 明确排序只选择具体字段和方向。
- PRD 明确需要新增的 feiwen 局部组件和可直接复用的 gpui-component 组件。
- PRD 明确图标、嵌套层级、错误状态和结果表格规则。
- 参考图包含 1 张总体图和 5 张局部状态图，且不展示旧搜索框和旧 tags 云。
