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
settings-save = 保存设置
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
recovery-confirm = 确定继续？重读成功后才会舍弃草稿；重置或覆盖会先备份当前文件。
recovery-backup = 备份位置
error-config-read = 无法读取配置，请检查文件位置和权限后重读。
error-config-parse = 配置格式无效。可修正文件后重读，或备份并重置。
error-config-validation = 请填写单个可执行文件名或绝对可执行路径，不附带参数。
error-config-conflict = 文件已被外部修改。请重读，或明确备份后用已提交的设置覆盖。
error-config-write = 保存失败，已保留草稿与已应用设置。提交结果不确定时请重读核对。
error-pi-probe = 无法验证 Pi。请检查可执行文件路径和 Node 环境后重试。
action-check-pi = 重新检查 Pi
action-reset = 备份并重置
action-overwrite = 备份并覆盖
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
conversation-scan-failed = 会话目录加载失败
conversation-search-placeholder = 搜索名称、消息或项目路径
conversation-search-empty = 没有匹配的会话
conversation-rename = 重命名…
conversation-copy-path = 复制路径
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
conversation-role-user = 用户消息
conversation-role-assistant = 助手消息
conversation-role-tool = 工具结果
conversation-event = 会话事件
conversation-close-history = 收起会话历史
conversation-source = 查看来源会话文件
conversation-reconnect = 重新连接
conversation-save-error = 草稿保存失败
conversation-recover-draft = 恢复未发送的输入
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
conversation-queued = 已排队，等待 Pi 执行
conversation-accepted = Pi 已接受
conversation-graph-current = 当前执行
conversation-graph-preview = 预览
conversation-graph-left = 查看左侧轨道（也可横向滚动）
conversation-graph-right = 查看右侧轨道（也可横向滚动）
conversation-graph-reveal = 定位选中
conversation-graph-column = 分支
conversation-graph-message = 消息
conversation-catalog-empty = 暂无会话

conversation-thinking-content = 思考过程
conversation-tool-group = 工具调用（{ $count }）
conversation-tool-group-running = 正在处理 { $count } 项工具调用
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
conversation-tool-group-explore = 查找并读取文件
conversation-tool-group-explore-commands = 读取文件并运行命令

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
conversation-history-brief = 简略：用户和助手消息
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
history-canvas-mode = 画布：可缩放的会话树
history-canvas-zoom-in = 放大
history-canvas-zoom-out = 缩小
history-canvas-fit = 查看全图（0）
history-canvas-current = 定位当前执行节点
history-canvas-expand = 展开 { $count } 个节点
history-canvas-collapse = 收起此连续段（{ $count } 个节点）
history-canvas-empty = 暂无可显示的历史节点
history-canvas-help = 会话树画布：滚动或拖动平移，捏合或加减号缩放，方向键选择，Enter 预览，E 展开下一段
