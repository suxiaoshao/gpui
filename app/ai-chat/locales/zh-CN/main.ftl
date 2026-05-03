app-title = AI 对话
settings-title = 设置
message-preview-title = 消息预览：{ $id }
app-menu-about = 关于 {$app_name}
app-menu-version = 版本（v{ $version }）
app-menu-open-main = 打开 AI 对话
app-menu-open-temporary = 打开临时对话
app-menu-settings = 设置
app-menu-services = 服务
app-menu-hide = 隐藏 {$app_name}
app-menu-hide-others = 隐藏其他
app-menu-show-all = 全部显示
app-menu-quit = 退出 {$app_name}
app-menu-window = 窗口
app-menu-minimize = 最小化
app-menu-zoom = 缩放
app-about-window-title = 关于 {$app_name}
app-about-version = 版本 {$version}
app-about-github = GitHub
tray-open-main = 打开 AI 对话
tray-open-temporary = 打开临时对话
tray-version = 版本（v{ $version }）
tray-about = 关于 AI 对话
tray-quit = 退出 AI 对话
tray-tooltip = AI 对话
tray-about-comments = 基于 GPUI 构建的桌面 AI 对话应用。
tray-about-website-label = 项目仓库

settings-page-general = 通用
settings-page-appearance = 外观
settings-page-provider = 提供方
settings-page-templates = 模板
settings-page-shortcuts = 快捷键
settings-group-basic-options = 基础选项
settings-group-shortcuts = 全局快捷键绑定
settings-templates-description = 在一个设置页中管理可复用的提示词模板。
settings-appearance-mode = 外观模式
settings-custom-theme-color = 自定义主题色
settings-custom-theme-color-description = 选择一个颜色生成 Material You 主题，并加入主题池。
settings-light-themes = 亮色主题
settings-dark-themes = 暗色主题
theme-selected = 已选择
theme-selected-prefix = 已选择：
appearance-mode-system = 跟随系统
appearance-mode-light = 亮色
appearance-mode-dark = 暗色
language-system = 跟随系统
language-english = English
language-chinese = 简体中文

sidebar-app-title = AI 对话
sidebar-conversation-tree = 会话树
sidebar-actions = 操作
sidebar-settings = 设置
sidebar-search-conversation = 搜索会话
sidebar-add-conversation = 新建会话
sidebar-add-folder = 新建文件夹
sidebar-root = 根目录

section-information = 信息
section-content = 内容
alert-error-title = 错误
message-status-normal = 正常
message-status-hidden = 已隐藏
message-status-loading = 加载中
message-status-thinking = 思考中
message-status-paused = 已暂停
message-status-error = 错误
role-developer = 开发者
role-user = 用户
role-assistant = 助手

button-add = 添加
button-add-prompt = 添加提示词
dialog-edit-folder-title = 编辑文件夹
dialog-edit-conversation-title = 编辑会话
notify-update-folder-failed = 更新文件夹失败
notify-update-conversation-failed = 更新会话失败

button-configure = 配置
button-cancel = 取消
button-create = 创建
button-delete = 删除
button-delete-template = 删除模板
button-edit = 编辑
button-export = 导出
button-view = 查看
button-close = 关闭
button-open = 打开
button-copy-to-new-conversation = 复制为新会话
button-clear = 清空对话
button-reload = 重新加载
button-preview = 预览
button-reset = 重置
button-regenerate = 重新生成
button-save = 保存对话
button-save-changes = 保存更改
button-save-message = 保存消息
button-save-shortcut = 保存快捷键
button-submit = 提交
button-add-material-theme = 添加 Material You 主题
button-delete-material-theme = 删除 Material You 主题

tooltip-copy = 复制
tooltip-delete = 删除
tooltip-view-detail = 查看详情
tooltip-resend-message = 重新发送消息
tooltip-send-message = 发送消息
tooltip-pause-message = 暂停生成
tooltip-clear-conversation = 清空对话
tooltip-save-conversation = 保存对话
tooltip-select-template = 选择模板
tooltip-show-api-key = 显示 API Key
tooltip-hide-api-key = 隐藏 API Key
tooltip-copy-conversation = 复制为新会话
tooltip-export-conversation = 导出会话
tooltip-ollama-web-search-help =
    Ollama 的 `web_search` 依赖 `web_fetch`。

    Ollama 的 `web_search` / `web_fetch` 走的是 cloud 能力，不是本地搜索插件开关。

    ## 1. 确认 Ollama 版本支持实验路由

    ```bash
    ollama --version
    ```

    如果下面这个请求返回 `404`，说明当前版本还不支持该路由，需要先升级 Ollama：

    ```bash
    curl http://localhost:11434/api/experimental/web_fetch \
      -H 'Content-Type: application/json' \
      -d '{"{"}"url":"https://ollama.com"{"}"}'
    ```

    ## 2. 登录 Ollama Cloud

    ```bash
    ollama signin
    ```

    本地 `localhost:11434` 路由走的是本机登录态；只有直连 `https://ollama.com/api/*` 时才需要 API key。

    ## 3. 确认没有禁用 cloud

    ```bash
    echo "$OLLAMA_NO_CLOUD"
    cat ~/.ollama/server.json
    ```

    确保：
    - `OLLAMA_NO_CLOUD` 不是 `1` / `true`
    - `~/.ollama/server.json` 里没有 `"disable_ollama_cloud": true`

    ## 4. 重启 Ollama

    修改登录态或 cloud 配置后，重启 Ollama。

    ## 5. 验证状态

    ```bash
    curl http://localhost:11434/api/status
    ```

    期望：
    - `cloud.disabled` 为 `false`

    再验证实验路由：

    ```bash
    curl http://localhost:11434/api/experimental/web_fetch \
      -H 'Content-Type: application/json' \
      -d '{"{"}"url":"https://ollama.com"{"}"}'
    ```

    结果判断：
    - `404`：当前 Ollama 版本不支持，先升级
    - `401`：还没登录，重新执行 `ollama signin`
    - `403`：cloud 被禁用，检查 `OLLAMA_NO_CLOUD` 和 `server.json`
    - `200`：可以使用

    官方文档：
    - [Web search](https://ollama.com/blog/web-search)
    - [API docs](https://ollama.com/api)
temporary-chat-title = 临时对话
temporary-chat-description = 开始一段临时对话，需要时可在聊天表单中选择模板。
temporary-chat-empty-title = 开始临时对话
temporary-chat-empty-description = 选择模板，或直接输入问题开始。

field-id = ID
field-name = 名称
field-icon = 图标
field-info = 说明
field-theme = 主题
field-language = 语言
field-config-file = 配置文件
field-http-proxy = HTTP 代理
field-temporary-conversation-hotkey = 临时会话快捷键
field-template = 模板
field-template-prefix = 模板
field-provider = 提供方
field-conversation-name = 会话名称
field-conversation-path = 会话路径
field-mode = 模式
field-description = 描述
field-enabled = 启用
field-none = 无
field-on = 开
field-off = 关
field-search-extension = 搜索扩展
field-search-conversation = 搜索会话
field-search-template = 搜索模板
field-search-models = 搜索模型
field-search-settings = 搜索设置
mode-contextual = 上下文模式
mode-single = 单轮模式
mode-assistant-only = 仅助手模式
field-prompts = 提示词
field-role = 角色
field-prompt = 提示词
field-chat-input-placeholder = 有问题，尽管问
field-model = 模型
field-preset = 预设
field-hotkey = 快捷键
field-thinking = 思考
field-reasoning-effort = 推理强度
field-reasoning-summary = 思考摘要
field-extension = 扩展
field-loading = 加载中...
field-models = 模型选择
field-web-search = 联网搜索
field-api-key = API Key
field-base-url = Base URL
field-sources = Sources
field-created-time = 创建时间
field-updated-time = 更新时间
field-start-time = 开始时间
field-end-time = 结束时间
field-status = 状态
field-error = 错误
field-text = 文本
field-citations = 引用
field-send-content = 发送内容
field-actions = 操作

dialog-add-folder-title = 新建文件夹
dialog-add-conversation-title = 新建会话
dialog-search-conversation-title = 搜索会话
dialog-view-template-title = 模板
dialog-edit-template-title = 编辑模板
dialog-add-template-title = 新建模板
dialog-delete-conversation-title = 删除会话
dialog-delete-conversation-message = 确定删除会话“{ $title }”吗？此操作无法撤销。
dialog-delete-folder-title = 删除文件夹
dialog-delete-folder-message = 确定删除文件夹“{ $name }”及其内容吗？此操作无法撤销。
dialog-delete-message-title = 删除消息
dialog-delete-message-message = 确定删除这条消息吗？此操作无法撤销。
dialog-clear-conversation-title = 清空对话
dialog-clear-conversation-message = 确定清空该对话中的所有消息吗？此操作无法撤销。
dialog-delete-temporary-message-title = 删除临时消息
dialog-delete-temporary-message-message = 确定删除这条临时消息吗？此操作无法撤销。
dialog-clear-temporary-chat-title = 清空临时对话
dialog-clear-temporary-chat-message = 确定清空该临时对话中的所有消息吗？此操作无法撤销。
dialog-delete-material-theme-title = 删除 Material You 主题
dialog-delete-material-theme-message = 确定删除这个自定义 Material You 主题吗？此操作无法撤销。
dialog-regenerate-message-title = 重新生成消息
dialog-regenerate-message-message = 确定重新生成这条助手消息吗？现有内容将被覆盖。
dialog-delete-template-title = 删除模板
dialog-delete-template-message = 确定删除该模板吗？此操作无法撤销。
dialog-add-shortcut-title = 新增快捷键
dialog-edit-shortcut-title = 编辑快捷键
dialog-delete-shortcut-title = 删除快捷键
dialog-delete-shortcut-message = 确定删除这条快捷键绑定吗？此操作无法撤销。

notify-get-templates-failed = 获取模板失败
notify-select-template = 请选择一个模板
notify-add-conversation-failed = 新建会话失败
notify-add-folder-failed = 新建文件夹失败
notify-move-conversation-failed = 移动会话失败
notify-move-folder-failed = 移动文件夹失败
notify-delete-message-failed = 删除消息失败
notify-delete-conversation-failed = 删除会话失败
notify-delete-folder-failed = 删除文件夹失败
notify-delete-template-failed = 删除模板失败
notify-template-deleted-success = 模板已删除
notify-load-template-failed = 加载模板失败
notify-load-templates-failed = 加载模板列表失败
notify-load-models-partial-failed = 部分模型提供方加载失败
notify-load-shortcuts-failed = 加载快捷键绑定失败
notify-load-template-schema-failed = 加载模板结构失败
notify-open-database-failed = 打开数据库失败
notify-reload-template-failed = 重新加载模板失败
notify-select-model = 请选择模型
notify-select-mode = 请选择模式
notify-select-adapter = 请选择适配器
notify-invalid-template = 模板配置无效
notify-invalid-prompts = 提示词无效
notify-invalid-shortcut-name = 快捷键名称不能为空
notify-invalid-shortcut-hotkey = 请输入有效的快捷键
notify-no-adapter-available = 没有可用适配器
notify-shortcut-created-success = 快捷键绑定创建成功
notify-shortcut-updated-success = 快捷键绑定更新成功
notify-shortcut-deleted-success = 快捷键绑定已删除
notify-shortcut-reregistered-success = 快捷键已重新注册
notify-shortcut-reregister-failed = 重新注册快捷键失败
notify-shortcut-trigger-busy-title = 快捷键正在执行
notify-shortcut-trigger-busy-message = 临时窗口当前仍在处理其他请求。
notify-shortcut-trigger-empty-input-title = 没有可发送内容
notify-shortcut-trigger-empty-input-message = 当前既没有可用的选中文字，也没有可用的剪贴板文本。
notify-shortcut-trigger-model-unavailable-title = 快捷键模型不可用
notify-shortcut-trigger-screenshot-title = 截图失败
notify-shortcut-trigger-ocr-title = 截图 OCR 失败
notify-save-shortcut-failed = 保存快捷键绑定失败
notify-delete-shortcut-failed = 删除快捷键绑定失败
notify-template-created-success = 模板创建成功
notify-template-updated-success = 模板更新成功
notify-create-template-failed = 模板创建失败
notify-update-template-failed = 模板更新失败
notify-update-message-success = 消息更新成功
notify-update-message-failed = 消息更新失败
notify-copy-success-title = 复制成功
notify-copy-success-message = 消息已复制到剪贴板。
notify-copy-failed-title = 复制失败
notify-copy-failed-message = 无法读取剪贴板内容。
notify-open-config-file-failed = 打开配置文件失败
notify-export-conversation-success = 会话已导出
notify-export-conversation-failed = 导出会话失败
empty-template-search = 没有匹配搜索条件的模板
conversation-search-no-results = 未找到会话。
settings-search-no-results = 未找到匹配的设置。

delete-message-failed-title = 删除消息失败
delete-message-failed-message = 消息窗口不可用。

settings-openai-title = OpenAI
settings-ollama-title = Ollama

reasoning-effort-none = 无
reasoning-effort-minimal = 最小
reasoning-effort-low = 低
reasoning-effort-medium = 中
reasoning-effort-high = 高
reasoning-effort-xhigh = 极高
button-reasoning-summary = 已思考
button-reasoning-summary-thinking = 思考中

template-error-select-role = 请选择提示词角色
template-error-prompt-empty = 提示词不能为空：
template-error-name-icon-required = 名称和图标不能为空

field-raw = 原始内容
empty-model-picker = 暂无可用模型
empty-template-picker = 暂无模板
empty-shortcut-bindings = 暂无快捷键绑定
shortcut-empty-search = 没有匹配的快捷键绑定
shortcut-search-placeholder = 搜索快捷键或模板名称
shortcut-settings-description = 管理把选中文字、剪贴板或截图 OCR 发送到临时对话的全局快捷键。
shortcut-filter-all = 全部
shortcut-filter-enabled = 已启用
shortcut-filter-disabled = 已停用
shortcut-filter-needs-action = 需处理
shortcut-filter-all-modes = 全部模式
shortcut-status-enabled = 已启用
shortcut-status-disabled = 已停用
shortcut-status-model-unavailable = 模型不可用
shortcut-status-hotkey-invalid = 热键无效
shortcut-status-hotkey-conflict = 热键冲突
shortcut-status-registration-failed = 注册失败
shortcut-status-message-enabled = 快捷键可用，等待触发。
shortcut-status-message-disabled = 该快捷键已停用。
shortcut-status-message-model-unavailable = 当前模型不在模型快照中。
shortcut-status-message-not-registered = 该快捷键尚未注册到系统热键服务。
shortcut-validation-temporary-conflict = 与临时会话快捷键冲突
shortcut-validation-binding-conflict = 与快捷键绑定冲突，ID:
shortcut-registration-registered = 已注册
shortcut-registration-not-registered = 未注册
shortcut-registration-disabled = 已停用，未注册
shortcut-runtime-waiting = 等待触发
shortcut-runtime-screenshot-active = 截图选择中
shortcut-status-dialog-title = 快捷键状态
shortcut-status-message = 状态说明
shortcut-status-registration = 快捷键注册
shortcut-status-runtime = 运行状态
shortcut-action-reload-models = 重新加载模型
shortcut-action-reregister = 重新注册快捷键
shortcut-unsaved-changes = 有未保存更改
shortcut-hotkey-placeholder = 按下快捷键
shortcut-preset-settings = 预设 / 扩展设置
shortcut-ext-settings-unavailable = 当前模型还不能解析能力信息，暂时无法编辑预设扩展项。
shortcut-model-unavailable = 不可用
send-content-selection-or-clipboard = 选中文字 / 剪贴板
send-content-screenshot = 截图
