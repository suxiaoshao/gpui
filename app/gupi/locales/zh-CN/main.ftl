app-title = Gupi
menu-settings = 设置
menu-show-main = 主窗口
menu-quit = 退出 Gupi
startup-welcome = 欢迎使用 Gupi
startup-checking = 正在检查…
startup-quitting = 正在完成操作…
settings-pi-command = Pi 可执行文件
settings-path-help = 留空以自动查找 pi，或填写可执行文件的绝对路径，不包含参数。
settings-theme = 外观
settings-language = 语言
settings-save-pi = 保存 Pi 路径
settings-reload = 从磁盘重读
settings-write-current = 将已应用设置写回磁盘（保留草稿）
theme-system = 跟随系统
theme-light = 浅色
theme-dark = 深色
language-system = 跟随系统
language-english = English
language-chinese = 简体中文
recovery-config-title = 配置需要处理
recovery-pi-title = 配置 Pi 环境
recovery-confirm = 确定继续？重读成功后才会舍弃草稿；重置会先备份当前文件。
recovery-backup = 备份位置
error-config-read = 无法读取配置，请检查文件位置和权限后重读。
error-config-parse = 配置格式无效。可修正文件后重读，或备份并重置。
error-config-validation = 请填写单个可执行文件名或绝对可执行路径，不附带参数。
error-config-write = 保存失败，草稿与已应用设置已保留。请检查配置文件后重试。
error-pi-probe = 无法验证 Pi。请检查可执行文件路径和 Node 环境后重试。
action-check-pi = 重新检查 Pi
action-reset = 备份并重置
action-confirm = 确认
action-cancel = 取消
home-ready = Pi 已就绪
home-description = 当前版本提供环境设置与桌面偏好。对话功能将在下一阶段接入。
action-locate = 显示配置文件夹
error-pi-timeout = Pi 未在 15 秒内完成版本检查。
error-pi-output = Pi 返回的内容超出了版本检查的输出限制。
error-pi-version = Pi 未正常退出，或未返回有效版本号。
error-log = 无法写入日志文件，请检查日志目录与权限。

setup-tagline = 为 Pi 准备的原生桌面空间
setup-intro = 简单设置，即可开始。
setup-start = 开始设置
setup-language-title = 选择你的语言
setup-search-language = 搜索语言…
setup-system-chinese = 系统语言：简体中文
setup-system-english = 系统语言：English
setup-appearance-title = 选择喜欢的外观
setup-color-mode = 亮暗模式
setup-system-accent = 系统强调色
setup-pi-title = 连接本机 Pi
setup-pi-step = Pi 配置
setup-pi-help = 连接已安装的本机 Pi。
setup-check-pi = 检测 Pi
setup-back = 上一步
setup-next = 下一步
setup-finish = 完成设置
setup-saving = 正在保存…
light-themes = 浅色主题
dark-themes = 深色主题

# Conversation workspace
conversation-new = 新建会话
conversation-sidebar = 会话侧边栏
conversation-history = 会话历史
conversation-restoring = 正在恢复草稿…
conversation-discovering = 正在查找会话… 已发现 { $count } 个文件
conversation-read-progress = 已读取 { $completed } / { $total } 个文件
conversation-refresh-progress = 正在刷新 { $completed } / { $total } 个文件
conversation-project = 选择工作目录
conversation-untitled = 未命名会话
conversation-idle = 空闲
conversation-loading = 正在打开
conversation-running = 正在运行
conversation-failed = 需要处理错误
conversation-waiting = 等待输入
conversation-search = 搜索会话
conversation-refresh = 刷新会话目录
conversation-refresh-current = 刷新当前会话
conversation-actions = 会话操作
conversation-show-sidebar = 展开会话侧栏
conversation-hide-sidebar = 收起会话侧栏
conversation-scan-failed = 会话目录加载失败
conversation-search-placeholder = 搜索名称、消息或项目路径
conversation-search-empty = 没有匹配的会话
conversation-rename = 重命名…
conversation-delete = 移到废纸篓
conversation-delete-failed = 无法删除会话：{ $error }
conversation-copy-path = 复制路径
conversation-clone = 复制会话
conversation-export = 导出会话为 HTML…
conversation-exported = 会话已导出至 { $path }
conversation-stop = 停止生成
conversation-close-run = 结束运行
conversation-show-less = 收起
conversation-show-more = 查看更多…
conversation-fork = 从这里另开会话
conversation-preview = 正在预览其他分支
conversation-return-current = 返回当前分支
conversation-welcome = 开始一段对话
conversation-welcome-hint = 选择工作目录，输入你的消息。
conversation-empty-history = 此会话还没有消息。
conversation-bottom = 返回底部
conversation-copy = 复制
conversation-compaction = 上下文压缩摘要
conversation-branch-summary = 分支摘要
conversation-working = 正在处理…
conversation-process = 查看处理过程
conversation-details = 展开或收起细节
tool-detail-offset = 起始行：
tool-detail-line-limit = 请求的行数上限：
tool-detail-timeout = 超时（秒）：
tool-detail-output = 输出
tool-detail-input = 输入
tool-detail-additional = 附加信息
tool-detail-error = 错误
tool-detail-glob = 文件匹配模式：
tool-detail-ignore-case = 忽略大小写：
tool-detail-literal = 按字面文本搜索：
tool-detail-context = 上下文行数：
tool-detail-result-limit = 请求的结果数量上限：
tool-detail-old-text = 请求匹配的原文
tool-detail-new-text = 请求替换的内容
tool-detail-truncated = Pi 已截断输出，当前显示的内容并非完整结果。
tool-detail-lines-truncated = Pi 已缩短部分结果行。
tool-detail-match-limit = 已达到匹配数量上限：
tool-detail-results-limited = 已达到结果数量上限：
tool-detail-entries-limited = 已达到目录条目数量上限：
tool-detail-full-output = 完整输出文件：
tool-detail-image-unavailable = 无法显示此图片。
conversation-role-user = 用户消息
conversation-role-assistant = 助手消息
conversation-role-tool = 工具结果
conversation-event = 会话事件
conversation-close-history = 收起会话历史
conversation-source = 查看来源会话文件
conversation-reconnect = 重新连接
conversation-save-error = 草稿保存失败
conversation-interrupted = 本次运行已停止；已保留现有内容。
conversation-no = 否
conversation-submit = 提交
conversation-input = 输入消息
conversation-model = 模型
conversation-load-options = 连接 Pi 以加载可用模型
conversation-thinking = 思考强度
conversation-unknown = 暂不可用
conversation-context = 当前执行分支上下文
conversation-auto-compaction = 自动压缩
conversation-on = 已开启
conversation-off = 已关闭
conversation-tokens = 累计输入 / 输出
conversation-cache = 累计缓存读取 / 写入
conversation-cache-hit = 最近缓存命中率
conversation-cost = Pi 统计费用
conversation-statistics = 用量统计
conversation-send = 发送（Enter）；Alt+Enter 等待本轮结束后发送
conversation-sending = 正在发送…
conversation-queued = 已排队，等待 Pi 执行
conversation-graph-current = 当前执行
conversation-graph-preview = 预览
conversation-graph-left = 查看左侧轨道（也可横向滚动）
conversation-graph-right = 查看右侧轨道（也可横向滚动）
conversation-graph-reveal = 定位选中
conversation-graph-column = 分支
conversation-graph-message = 消息
conversation-catalog-empty = 暂无会话

conversation-thinking-content = 思考过程
conversation-thinking-running = 正在思考
conversation-tool-group = 工具调用（{ $count }）
conversation-tool-group-read = 读取文件（{ $count }）
conversation-tool-group-bash = 运行命令（{ $count }）
conversation-tool-group-search = 查找内容（{ $count }）
conversation-tool-group-edit = 修改文件（{ $count }）
conversation-tool-line = { $action } { $summary }
conversation-tool-action-read =
    { $state ->
        [running] 正在读取
        [complete] 已读取
        [failed] 读取失败
        *[unfinished] 未完成读取
    }
conversation-tool-action-write =
    { $state ->
        [running] 正在写入
        [complete] 已写入
        [failed] 写入失败
        *[unfinished] 未完成写入
    }
conversation-tool-action-edit =
    { $state ->
        [running] 正在编辑
        [complete] 已编辑
        [failed] 编辑失败
        *[unfinished] 未完成编辑
    }
conversation-tool-action-bash =
    { $state ->
        [running] 正在运行
        [complete] 已运行
        [failed] 运行失败
        *[unfinished] 未完成运行
    }
conversation-tool-action-search =
    { $state ->
        [running] 正在查找
        [complete] 已查找
        [failed] 查找失败
        *[unfinished] 未完成查找
    }
conversation-tool-action-other =
    { $state ->
        [running] 正在调用 { $name }
        [complete] 已调用 { $name }
        [failed] 调用失败 { $name }
        *[unfinished] 未完成调用 { $name }
    }

conversation-processed = 已处理 { $duration }
conversation-processed-failed = 处理失败 { $duration }
conversation-processed-stopped = 已停止 { $duration }
conversation-copied = 已复制
conversation-copy-failed = 复制失败，请重试
conversation-usage-title = 本次请求用量
conversation-usage-model = 模型
conversation-usage-provider = 服务商
conversation-usage-input = 输入 Token
conversation-usage-output = 输出 Token
conversation-usage-cache-read = 缓存读取 Token
conversation-usage-cache-write = 缓存写入 Token
conversation-usage-total = 总 Token
conversation-usage-cost = 费用

composer-context-used = 已占用 Token
composer-context-limit = 上下文容量
composer-context-percent = 占用比例
composer-token-input = 累计输入 Token
composer-token-output = 累计输出 Token
composer-token-cache-read = 累计缓存读取 Token
composer-token-cache-write = 累计缓存写入 Token
composer-token-usage = 会话 Token 用量

conversation-model-search = 搜索模型…
conversation-model-empty = 没有匹配的模型
conversation-thinking-off = 关闭
conversation-thinking-minimal = 最小
conversation-thinking-low = 低
conversation-thinking-medium = 中
conversation-thinking-high = 高
conversation-thinking-xhigh = 超高
conversation-thinking-max = 最大

conversation-model-reasoning = 推理
conversation-model-vision = 视觉

conversation-history-reply = 助手回复
conversation-history-brief = 简略：用户消息与最终回答
conversation-history-detailed = 详细：消息、工具和摘要
conversation-history-progress = 助手过程
conversation-history-thinking = 思考
conversation-history-calls = 工具调用
conversation-history-failed = 执行失败
conversation-history-stopped = 已中断
conversation-history-empty = 无正文回复
composer-model-thinking = 模型与思考程度
composer-model-refresh = 刷新模型与思考档位
composer-thinking-unavailable = 不支持思考调节
composer-model-loading = 正在读取模型列表…
composer-thinking-loading = 正在读取思考档位…
composer-model-confirming = 正在确认模型设置…
composer-stats-loading = 正在读取用量…
composer-stats-retry = 重新读取用量
conversation-checking-file = 正在检查会话文件…
conversation-connecting = 正在连接 Pi…
conversation-history-loading = 正在读取会话历史…
conversation-history-refreshing = 正在刷新会话历史…
conversation-fork-options-loading = 正在读取另开会话选项…
conversation-fork-options-retry = 重新读取另开会话选项
history-canvas-zoom-in = 放大
history-canvas-zoom-out = 缩小
history-canvas-fit = 查看全图（0）
history-canvas-current = 定位当前执行节点
history-canvas-expand = 展开 { $count } 个节点
history-canvas-collapse = 收起 { $count } 个过程节点，保留当前定位
history-canvas-empty = 暂无可显示的历史节点
history-canvas-help = 会话树画布：滚动或拖动平移，捏合或加减号缩放，方向键选择，Enter 预览，E 展开下一段

history-view-tree = 树
history-view-list = 列表
history-level-brief = 简略
history-level-detailed = 详细
history-level-all = 全部
history-level-all-description = 全部：包括设置变化、标签和自定义记录
history-scope-all = 全部分支
history-scope-branch = 仅此分支
history-model-change = 模型变更
history-thinking-change = 思考级别变更
history-session-info = 会话信息变更
history-label-change = 标签变更
history-custom-record = 自定义记录


history-content = 内容
history-range = 范围

command-palette = 命令面板
command-focus-input = 聚焦对话输入
command-model = 选择模型与思考等级…
command-copy-last-answer = 复制最后回答
command-compact = 压缩上下文
conversation-compacting = 正在压缩上下文…
command-history-description = 查看历史，或选择用户消息分叉
command-show-history = 显示历史
command-hide-history = 隐藏历史
command-current-session = 当前会话：
command-unavailable = 当前状态不可用
command-target-changed = 此会话已不可用，请选择其他会话。
command-scanning = 正在加载会话…
command-loading = 正在加载 Pi 命令…
command-no-matches = 没有匹配的 Pi 命令
conversation-reconnect-unconfirmed = 无法确认 Pi 进程已退出，请重启 Gupi 后再连接。
action-retry = 重试

command-search-placeholder = 搜索操作或输入命令
command-empty = 当前会话没有 Pi 命令

command-group-app = 应用
command-group-extensions = 插件命令
command-group-skills = 技能
command-group-prompts = 提示词模板
command-scope-user = 个人
command-scope-project = 项目
command-scope-temporary = 临时
command-connection-unavailable = Pi 尚未就绪
command-dismiss = 退出
command-complete = 补全
command-send-text = 发送
command-execute = 执行

command-scope-current = 当前会话

conversation-working-duration = 正在处理 { $duration }
conversation-tool-group-skill = 读取技能（{ $count }）
conversation-shell-line = { $shell } · { $action } { $summary }
conversation-tool-action-skill =
    { $state ->
        [running] 正在读取技能
        [complete] 已读取技能
        [failed] 技能读取失败
       *[unfinished] 技能读取未完成
    }

# Unified settings
settings-page-general = 通用
settings-page-pi = Pi
settings-page-keys = 快捷键
settings-page-plugins = 插件
settings-page-skills = Skill
settings-page-prompts = 提示词
settings-page-about = 关于
settings-key-reset = 恢复默认
settings-key-reset-all = 全部恢复默认
settings-key-reset-all-confirm = 恢复所有快捷键的默认绑定？所有自定义快捷键将被清除。
settings-key-clear = 清除快捷键
settings-key-cancel = 取消本次修改
settings-key-unbound = 未设置快捷键
settings-key-record = 录制
settings-key-recording = 按下快捷键…
settings-key-conflict = 快捷键冲突或无效
settings-key-invalid = 请输入有效的快捷键；留空表示取消绑定
settings-resource-refresh = 重新读取资源
settings-resource-reload-help = 仅管理个人资源。修改后，请手动刷新需要更新的会话。
settings-resource-working = 正在处理…
settings-resource-pi-required = 请先在 Pi 设置中保存并检查可用的 Pi 命令。
settings-resource-invalid-name = 名称只能包含英文字母、数字、连字符或下划线。
settings-resource-register = 添加本地
settings-resource-create = 创建
settings-resource-name-help = 新资源名称：英文字母、数字、连字符或下划线。
settings-resource-empty = 没有匹配的资源。
settings-resource-delete = 移到废纸篓
settings-resource-save = 保存正文
settings-resource-loading = 正在读取正文…
settings-resource-confirm-remove = 确认移除此项？删除文件仅将所选文件移到废纸篓，不会删除整个目录。
settings-package-install = 安装包
settings-package-update = 更新
settings-package-remove = 移除
settings-package-source-help = 包来源：npm:包名、Git 地址或本地绝对路径。
settings-package-extensions = 扩展 { $count }
settings-package-skills = Skill { $count }
settings-package-prompts = 模板 { $count }
settings-package-themes = 主题 { $count }
settings-skill-search = 搜索 Skill 名称、说明、路径或来源
settings-skill-collapse = 收起
settings-resource-edit = 编辑
settings-about-gupi = 版本
settings-about-pi = 版本
settings-about-path = 实际路径
settings-about-status = 检查状态
settings-about-unavailable = 尚未取得
settings-editor-discard = 放弃未保存的正文修改吗？
settings-package-confirm-remove = 从个人 Pi 中移除此包？本地包只取消注册，保留来源目录。
settings-package-heading = 已安装包
settings-extension-heading = 独立扩展
settings-template-heading = 命令模板
settings-system-heading = 系统提示词
settings-config-heading = 配置文件
settings-config-open = 打开配置文件
settings-mode-help = 跟随系统时，自动使用对应的浅色或深色主题。
settings-light-help = 浅色模式下使用此主题。
settings-dark-help = 深色模式下使用此主题。
settings-pi-unsaved = 路径尚未保存；检查仅验证当前输入，不会保存修改。
settings-pi-saved = 当前路径已保存。检查可重新确认 Pi 是否可用。
settings-resource-location = 显示位置
settings-package-source = 包来源
settings-package-empty = 尚未安装个人 Pi 包。
settings-resource-name = 名称
settings-resource-none = 尚无此类个人资源。
settings-resource-view = 查看
settings-resource-readonly-badge = 只读
settings-source-personal = 个人
settings-source-external = 外部来源
settings-system-replace = 替换默认系统提示词
settings-system-replace-help = 完整替换 Pi 的默认系统提示词。
settings-system-append = 追加个人指令
settings-system-append-help = 保留默认系统提示词，并在末尾追加这些指令。
settings-resource-success = 已完成对“{ $target }”的操作。已有会话需手动刷新。
settings-resource-failed = 对“{ $target }”的操作失败。
settings-about-help = 显示已保存路径最近一次检查的结果；如需重新检查，请进入 Pi 设置。
settings-key-reveal-session = 显示会话文件

settings-key-group-app = 应用
settings-key-group-conversation = 会话
settings-key-group-files = 会话文件

settings-package-kind-extensions = 扩展
settings-package-kind-skills = Skill
settings-package-kind-themes = 主题
settings-package-kind-prompts = 模板
settings-package-expand = 展开包内容
settings-package-collapse = 收起包内容

settings-template-search = 搜索模板名称、说明或来源
settings-template-collapse = 收起预览
