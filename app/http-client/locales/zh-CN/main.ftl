app-title = HTTP 客户端

gpui-form-error-required = 此字段为必填项。

button-send = 发送
button-cancel = 取消
button-clear-response = 清除响应
button-save-response = 保存响应
button-add = 添加
button-confirm = 确认
button-delete = 删除
button-select-file = 选择文件
button-change-file = 更换文件
button-clear-file = 清除文件
button-move-up = 上移
button-move-down = 下移

field-method = 方法
field-url = 链接
field-name = 名称
field-key = 键
field-value = 值
field-content-type = 内容类型
field-file = 文件
field-username = 用户名
field-password = 密码
field-token = 令牌
field-location = 位置
field-timeout-ms = 超时（毫秒）

tab-params = 参数
tab-authorization = 授权
tab-headers = 请求头
tab-body = 请求体
tab-settings = 设置
tab-response-body = 响应体
tab-response-headers = 响应头

response-title = 响应
response-empty = 发送请求后可在此查看响应。
response-sending = 正在发送请求…
response-receiving-known = 正在接收响应：{ $received } / { $total }
response-receiving-unknown = 正在接收响应：{ $received }
response-status = 状态
response-final-url = 最终链接
response-protocol = 协议
response-head-time = 响应头用时
response-total-time = 总用时
response-received-size = 已接收
response-stored-size = 已保存
response-header-name = 响应头
response-header-value = 值
response-headers-empty = 该响应没有响应头。

response-view-auto = 自动
response-view-text = 文本
response-view-json = JSON
response-view-xml = XML
response-view-hex = 十六进制
response-view-base64 = Base64
response-view-image = 图片

response-preview-truncated = 预览已截断。请保存响应以查看全部字节。
response-decoding-unsupported = 不支持该内容编码，已保留编码后的字节。
response-viewer-mode-unavailable = 当前响应无法使用该视图。
response-viewer-invalid-json = 响应不是有效 JSON，已改为显示受限文本。
response-viewer-invalid-image = 无法将响应解码为受支持的图片。
response-image-too-large = 图片超出安全预览限制。
response-save-complete = 响应已保存。
response-save-failed = 无法保存响应。

request-problem-transport = 请求无法连接到服务器。
request-problem-timeout = 请求已超时。
request-problem-redirect = 无法完成重定向链。
request-problem-request-body = 无法读取请求体。
request-problem-response-read = 无法完整读取响应体。
request-problem-response-decode = 无法完整解码响应内容。
request-problem-storage = 无法安全保存响应。
request-problem-too-large-encoded = 编码后的响应过大（{ $observed } 字节，上限 { $limit } 字节）。
request-problem-too-large-stored = 解码后的响应过大（{ $observed } 字节，上限 { $limit } 字节）。
request-problem-internal = 请求因内部错误结束。

params-invalid-url-disabled = 请输入有效的 HTTP 或 HTTPS 绝对链接后再编辑查询参数。

body-none = 无
body-form-data = form-data
body-urlencoded = x-www-form-urlencoded
body-text = 文本
body-binary = 二进制文件

text-format-plain = 纯文本
text-format-json = JSON
text-format-javascript = JavaScript
text-format-html = HTML
text-format-xml = XML
text-format-css = CSS

multipart-text = 文本
multipart-file = 文件
multipart-file-not-selected = 未选择文件

auth-none = 无授权
auth-basic = 基本身份验证
auth-bearer = Bearer 令牌
auth-api-key = API 密钥
auth-location-header = 请求头
auth-location-query = 查询参数
auth-generated-override = 生成的授权信息优先于请求中冲突的值。
auth-query-override = API 密钥授权会覆盖名为“{ $name }”的查询参数。
body-content-type-override = 此显式请求头优先于请求体生成的 Content-Type。

settings-follow-redirects = 跟随重定向
settings-follow-original-method = 跟随重定向时保留原请求方法
settings-timeout-help = 设为 0 表示不限时。

request-url-invalid = 请输入带主机名的有效绝对链接。
request-url-scheme-invalid = 仅支持 HTTP 和 HTTPS 链接。
request-header-name-invalid = 请输入有效的 HTTP 请求头名称。
request-header-value-invalid = 请输入有效的 HTTP 请求头值。
request-media-type-invalid = 请输入有效的媒体类型。
request-multipart-name-required = 请输入该 multipart 部分的名称。
request-multipart-name-invalid = multipart 名称不能包含换行或空字符。
request-file-required = 请选择文件。
request-file-unavailable = 所选文件不可用、不可读或不是普通文件。
request-file-name-invalid = 文件名不能为空，也不能包含换行或空字符。
request-basic-username-colon = 基本身份验证用户名不能包含冒号。
request-auth-value-invalid = 请输入可用于 HTTP 授权请求头的值。
request-api-key-name-required = 请输入 API 密钥名称。
request-api-key-name-invalid = 请输入适用于所选位置的有效 API 密钥名称。
