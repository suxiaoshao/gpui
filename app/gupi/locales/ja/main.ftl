app-title = Gupi
menu-settings = 設定
menu-show-main = メインウィンドウ
menu-quit = Gupi を終了
startup-welcome = Gupi へようこそ
temporary-setup-required = 設定でセットアップを完了すると、一時会話を開始できます。
startup-checking = 確認中…
startup-quitting = 処理を終了しています…
settings-pi-command = Pi の実行ファイル
settings-path-help = 空欄にすると pi を自動検索します。引数なしの実行ファイルの絶対パスも指定できます。
settings-theme = 外観
settings-language = 言語
settings-save-pi = Pi のパスを保存
settings-reload = ディスクから再読み込み
settings-write-current = 適用中の設定をディスクに書き込む（編集中の内容は保持）
theme-system = システム
theme-light = ライト
theme-dark = ダーク
language-system = システム
language-english = English
language-chinese = 简体中文
recovery-config-title = 設定を確認してください
recovery-pi-title = Pi を設定
recovery-confirm = 続行しますか？再読み込みに成功した場合のみ編集内容は破棄されます。リセット前に現在のファイルをバックアップします。
recovery-backup = バックアップ
error-config-read = 設定を読み込めませんでした。場所とアクセス権を確認してから再読み込みしてください。
error-config-parse = 設定の形式が正しくありません。修正したファイルを再読み込みするか、バックアップしてリセットしてください。
error-config-validation = 実行ファイル名を1つ、または引数なしの実行ファイルの絶対パスを入力してください。
error-config-write = 保存に失敗しました。編集中の内容と適用済みの設定は保持されています。設定ファイルを確認して再試行してください。
error-pi-probe = Pi を確認できませんでした。実行ファイルのパスと Node の環境を確認して、もう一度お試しください。
action-check-pi = Pi を再確認
action-reset = バックアップしてリセット
action-confirm = 確認
action-cancel = キャンセル
home-ready = Pi を利用できます
home-description = このバージョンでは環境設定とデスクトップ設定を利用できます。会話機能は次の段階で提供予定です。
action-locate = 設定フォルダーを表示
error-pi-timeout = Pi のバージョン確認が15秒以内に完了しませんでした。
error-pi-output = Pi の出力がバージョン確認の上限を超えました。
error-pi-version = Pi が正常に終了しなかったか、有効なバージョンを返しませんでした。
error-log = ファイルへのログ記録を利用できません。ログの保存先とアクセス権を確認してください。

setup-tagline = Pi のためのネイティブデスクトップ
setup-intro = 簡単な設定ですぐに始められます。
setup-start = 始める
setup-language-title = 使いやすい言語を選択
setup-search-language = 言語を検索…
setup-system-language = システム言語：{ $language }
setup-appearance-title = 外観をカスタマイズ
setup-color-mode = カラーモード
setup-system-accent = システムのアクセントカラー
setup-pi-title = Pi に接続
setup-pi-step = Pi のセットアップ
setup-pi-help = ローカルの Pi を確認するか、このページをスキップして後から設定できます。
setup-check-pi = Pi を確認
setup-back = 戻る
setup-next = 続行
setup-finish = セットアップを完了
setup-saving = 保存中…
light-themes = ライトテーマ
dark-themes = ダークテーマ

# Conversation workspace
conversation-new = 新しい会話
conversation-sidebar = 会話サイドバー
conversation-history = 会話履歴
conversation-restoring = 下書きを復元中…
conversation-discovering = 会話を検索中… ファイル数 { $count }
conversation-read-progress = ファイルを読み込み中 { $completed} / { $total}
conversation-refresh-progress = 更新中 { $completed} / { $total}
conversation-project = 作業フォルダーを選択
conversation-untitled = 無題の会話
conversation-idle = 待機中
conversation-loading = セッションを開いています
conversation-running = 実行中
conversation-failed = エラーが発生しました
conversation-waiting = 入力待ち
conversation-search = 会話を検索
conversation-refresh = セッションを更新
conversation-refresh-current = 現在のセッションを更新
conversation-actions = セッションの操作
conversation-show-sidebar = セッションサイドバーを表示
conversation-hide-sidebar = セッションサイドバーを非表示
conversation-scan-failed = 会話カタログを読み込めませんでした
conversation-search-placeholder = 名前、メッセージ、プロジェクトのパスを検索
conversation-search-empty = 一致する会話はありません
conversation-rename = 名前を変更…
conversation-delete = ゴミ箱に移動
conversation-delete-failed = 会話を削除できませんでした：{ $error }
conversation-copy-path = パスをコピー
conversation-clone = 会話を複製
conversation-export = 会話を HTML として書き出す…
conversation-exported = 会話を { $path} に書き出しました
conversation-stop = 生成を停止
conversation-close-run = ランタイムを閉じる
conversation-show-less = 折りたたむ
conversation-show-more = さらに表示…
conversation-fork = ここから会話を分岐
conversation-preview = 別のブランチをプレビュー中
conversation-return-current = 現在のブランチに戻る
conversation-welcome = 会話を始めましょう
conversation-welcome-project = { $project} で何をしますか？
conversation-empty-history = この会話にはまだメッセージがありません。
conversation-bottom = 一番下へ移動
conversation-copy = コピー
conversation-compaction = コンテキストの圧縮概要
conversation-branch-summary = ブランチの概要
conversation-working = 処理中…
conversation-process = プロセスを表示
conversation-details = 詳細を切り替え
tool-detail-offset = 開始行：
tool-detail-line-limit = 要求された行数：
tool-detail-timeout = タイムアウト（秒）：
tool-detail-output = 出力
tool-detail-input = 入力
tool-detail-additional = 追加情報
tool-detail-error = エラー
tool-detail-glob = ファイルパターン：
tool-detail-ignore-case = 大文字と小文字を区別しない：
tool-detail-literal = リテラル検索：
tool-detail-context = 前後に表示する行数：
tool-detail-result-limit = 要求された結果数：
tool-detail-old-text = 置換前のテキスト
tool-detail-new-text = 置換後のテキスト
tool-detail-truncated = Pi の出力は一部省略されています。表示内容は全体ではありません。
tool-detail-lines-truncated = 一部の結果行が短縮されました。
tool-detail-match-limit = 一致数の上限に達しました：
tool-detail-results-limited = 結果数の上限に達しました：
tool-detail-entries-limited = ディレクトリエントリ数の上限に達しました：
tool-detail-full-output = 出力ファイル全体：
tool-detail-image-unavailable = この画像を表示できません。
conversation-role-user = ユーザーメッセージ
conversation-role-assistant = アシスタントのメッセージ
conversation-role-tool = ツールの結果
conversation-event = セッションイベント
conversation-close-history = 会話履歴を閉じる
conversation-source = 元のセッションファイルを表示
conversation-reconnect = 再接続
conversation-save-error = 下書きを保存できませんでした
conversation-interrupted = 実行を停止しました。既存の内容は保持されています。
conversation-no = いいえ
conversation-submit = 送信
conversation-input = メッセージを入力
conversation-model = モデル
conversation-load-options = モデルを読み込むには Pi に接続してください
conversation-thinking = 思考レベル
conversation-unknown = 利用できません
conversation-context = 現在の実行ブランチのコンテキスト
conversation-auto-compaction = 自動圧縮
conversation-on = オン
conversation-off = オフ
conversation-tokens = 入力 / 出力の合計
conversation-cache = キャッシュ読み込み / 書き込みの合計
conversation-cache-hit = 最新のキャッシュヒット率
conversation-cost = Pi が報告したコスト
conversation-statistics = 使用状況
conversation-send = 送信（Enter）；Alt+Enter で追加の依頼をキューに追加
conversation-sending = 送信中…
conversation-graph-current = 現在
conversation-graph-preview = プレビュー
conversation-graph-left = 左側のレーンを表示（または横にスクロール）
conversation-graph-right = 右側のレーンを表示（または横にスクロール）
conversation-graph-reveal = 選択項目を表示
conversation-graph-column = グラフ
conversation-graph-message = メッセージ
conversation-catalog-empty = 会話はまだありません

conversation-thinking-content = 思考
conversation-thinking-running = 思考中…
conversation-tool-group = ツール呼び出し（{ $count }）
conversation-tool-group-read = ファイルを読み込み（{ $count }）
conversation-tool-group-bash = コマンド（{ $count }）
conversation-tool-group-search = 検索（{ $count }）
conversation-tool-group-edit = ファイルの変更（{ $count }）
conversation-tool-line = { $action } { $summary }
conversation-tool-action-read = { $state ->
    [running] 読み込み中
    [complete] 読み込み済み
    [failed] 読み込みに失敗
   *[unfinished] 読み込み未完了
}
conversation-tool-action-write = { $state ->
    [running] 書き込み中
    [complete] 書き込み済み
    [failed] 書き込みに失敗
   *[unfinished] 書き込み未完了
}
conversation-tool-action-edit = { $state ->
    [running] 編集中
    [complete] 編集済み
    [failed] 編集に失敗
   *[unfinished] 編集未完了
}
conversation-tool-action-bash = { $state ->
    [running] 実行中
    [complete] 実行済み
    [failed] コマンドに失敗
   *[unfinished] コマンド実行未完了
}
conversation-tool-action-search = { $state ->
    [running] 検索中
    [complete] 検索済み
    [failed] 検索に失敗
   *[unfinished] 検索未完了
}
conversation-tool-action-other = { $state ->
    [running] { $name} を呼び出し中
    [complete] { $name} を呼び出し済み
    [failed] { $name} の呼び出しに失敗
   *[unfinished] { $name} の呼び出し未完了
}

conversation-processed = { $duration} 実行しました
conversation-processed-failed = { $duration} 後に失敗
conversation-processed-stopped = { $duration} 後に停止
conversation-copied = コピーしました
conversation-copy-failed = コピーできませんでした。もう一度お試しください。
conversation-usage-title = リクエストの使用量
conversation-usage-model = モデル
conversation-usage-provider = プロバイダー
conversation-usage-input = 入力トークン
conversation-usage-output = 出力トークン
conversation-usage-cache-read = キャッシュ読み込みトークン
conversation-usage-cache-write = キャッシュ書き込みトークン
conversation-usage-total = 合計トークン
conversation-usage-cost = コスト

composer-context-used = 使用済みトークン
composer-context-limit = コンテキスト容量
composer-context-percent = コンテキストの使用量
composer-token-input = 入力トークンの合計
composer-token-output = 出力トークンの合計
composer-token-cache-read = キャッシュ読み込みトークンの合計
composer-token-cache-write = キャッシュ書き込みトークンの合計
composer-token-usage = セッションのトークン使用量

conversation-model-search = モデルを検索…
conversation-model-empty = 一致するモデルはありません
conversation-thinking-off = オフ
conversation-thinking-minimal = 最小
conversation-thinking-low = 低
conversation-thinking-medium = 中
conversation-thinking-high = 高
conversation-thinking-xhigh = 非常に高い
conversation-thinking-max = 最大

conversation-model-reasoning = 推論
conversation-model-vision = 画像認識

conversation-history-reply = アシスタントの返信
conversation-history-brief = 簡易：ユーザーメッセージと最終回答
conversation-history-detailed = 詳細：メッセージ、ツール、概要
conversation-history-progress = アシスタントの進捗
conversation-history-thinking = 思考
conversation-history-calls = ツール呼び出し
conversation-history-failed = 実行に失敗
conversation-history-stopped = 中断
conversation-history-empty = テキストのない返信
composer-model-thinking = モデルと思考レベル
composer-model-refresh = モデルと思考レベルを更新
composer-thinking-unavailable = 非対応
composer-model-loading = モデルを読み込み中…
composer-thinking-loading = 思考レベルを読み込み中…
composer-model-confirming = モデル設定を確認中…
composer-stats-loading = 使用状況を読み込み中…
composer-stats-retry = 使用状況の読み込みを再試行
conversation-checking-file = 会話ファイルを確認中…
conversation-connecting = Pi に接続中…
conversation-history-loading = 会話履歴を読み込み中…
conversation-history-refreshing = 会話履歴を更新中…
conversation-fork-options-loading = 分岐のオプションを読み込み中…
conversation-fork-options-retry = 分岐オプションの読み込みを再試行
history-canvas-zoom-in = 拡大
history-canvas-zoom-out = 縮小
history-canvas-fit = ツリー全体に合わせる（0）
history-canvas-current = 実行ノードに移動
history-canvas-expand = ノードを { $count } 件展開
history-canvas-collapse = 移動位置を残して { $count } 個のプロセスノードを折りたたむ
history-canvas-empty = 表示する履歴ノードがありません
history-canvas-help = 会話ツリー：スクロールまたはドラッグで移動、ピンチまたは +/- キーで拡大縮小、矢印キーで選択、Enter でプレビュー、E で次の区間を展開

history-view-tree = ツリー
history-view-list = リスト
history-level-brief = 簡易
history-level-detailed = 詳細
history-level-all = すべて
history-level-all-description = すべて：設定、ラベル、カスタム記録を含む
history-scope-all = すべてのブランチ
history-scope-branch = このブランチのみ
history-model-change = モデルの変更
history-thinking-change = 思考レベルの変更
history-session-info = セッション情報の変更
history-label-change = ラベルの変更
history-custom-record = カスタム記録


history-content = 内容
history-range = 範囲

command-palette = コマンドパレット
command-focus-input = 会話入力欄にフォーカス
command-model = モデルと思考レベルを選択…
command-copy-last-answer = 直前の回答をコピー
command-compact = コンテキストを圧縮
conversation-compacting = コンテキストを圧縮中…
command-history-description = 履歴を表示するか、分岐元のユーザーメッセージを選択
command-show-history = 履歴を表示
command-hide-history = 履歴を非表示
command-current-session = 現在のセッション：
command-unavailable = 現在の状態では利用できません
command-target-changed = この会話は利用できなくなりました。別の会話を選択してください。
command-scanning = 会話を読み込み中…
command-loading = Pi のコマンドを読み込み中…
command-no-matches = 一致する Pi コマンドはありません
conversation-reconnect-unconfirmed = Pi プロセスの終了を確認できませんでした。再接続する前に Gupi を再起動してください。
action-retry = 再試行

command-search-placeholder = アクションを検索するか、コマンドを入力
command-empty = このセッションでは Pi コマンドを利用できません

command-group-app = アプリケーション
command-group-extensions = 拡張機能のコマンド
command-group-skills = スキル
command-group-prompts = プロンプトテンプレート
command-scope-user = 個人
command-scope-project = プロジェクト
command-scope-temporary = 一時
command-connection-unavailable = Pi はまだ準備できていません
command-dismiss = 閉じる
command-complete = 完了
command-send-text = 送信
command-execute = 実行

command-scope-current = 現在のセッション

conversation-working-duration = 作業中 { $duration }
conversation-tool-group-skill = 読み込んだスキル（{ $count }）
conversation-shell-line = { $shell} · { $action } { $summary}
conversation-tool-action-skill = { $state ->
    [running] スキルを読み込み中
    [complete] スキルを読み込み済み
    [failed] スキルの読み込みに失敗
   *[unfinished] スキルの読み込み未完了
}

# Unified settings
settings-page-general = 一般
settings-page-pi = Pi
settings-page-keys = キーボードショートカット
settings-page-plugins = プラグイン
settings-page-skills = スキル
settings-page-prompts = プロンプト
settings-page-about = このアプリについて
settings-key-reset = 既定に戻す
settings-key-reset-all = すべての既定値に戻す
settings-key-reset-all-confirm = すべてのショートカットを既定の割り当てに戻しますか？カスタムの割り当てはすべて削除されます。
settings-key-clear = ショートカットを解除
settings-key-cancel = 変更を取り消す
settings-key-unbound = ショートカットが割り当てられていません
settings-key-record = 記録
settings-key-recording = ショートカットを押してください…
settings-key-conflict = ショートカットが競合しているか、無効です
settings-key-invalid = 有効なショートカットを入力するか、空欄にして割り当てを解除してください
settings-resource-refresh = リソースを更新
settings-resource-reload-help = 個人用リソースのみです。変更を適用するには既存のセッションを手動で再読み込みしてください。
settings-resource-working = 処理中…
settings-resource-pi-required = まず Pi の設定で、Pi の実行ファイルを保存して動作確認してください。
settings-resource-invalid-name = 名前には英字、数字、ハイフン、アンダースコアを使用できます。
settings-resource-register = ローカルに追加
settings-resource-create = 作成
settings-resource-name-help = 新しいリソース名：英字、数字、ハイフン、アンダースコア。
settings-resource-empty = 一致するリソースはありません。
settings-resource-delete = ゴミ箱に移動
settings-resource-save = テキストを保存
shortcut-save-task = 保存
settings-resource-loading = テキストを読み込み中…
settings-resource-confirm-remove = この項目を削除しますか？ファイルだけをゴミ箱に移動し、フォルダーは残します。
settings-package-install = パッケージをインストール
settings-package-update = 更新
settings-package-remove = 削除
settings-package-source-help = パッケージの取得元：npm:name、Git URL、またはローカルの絶対パス。
settings-package-extensions = 拡張機能 { $count}
settings-package-skills = スキル { $count}
settings-package-prompts = テンプレート { $count}
settings-package-themes = テーマ { $count}
settings-skill-search = 名前、説明、パス、取得元でスキルを検索
settings-skill-collapse = 折りたたむ
settings-resource-edit = 編集
settings-about-gupi = バージョン
settings-about-pi = バージョン
settings-about-path = 解決済みのパス
settings-about-status = 確認状況
settings-about-unavailable = まだ利用できません
settings-editor-discard = 保存されていないテキストの変更を破棄しますか？
settings-package-confirm-remove = 個人用の Pi 設定からこのパッケージを削除しますか？ローカルのパッケージソースフォルダーは保持されます。
settings-package-heading = インストール済みパッケージ
settings-extension-heading = 単独の拡張機能
settings-template-heading = コマンドテンプレート
settings-system-heading = システムプロンプト
settings-config-heading = 設定ファイル
settings-config-open = 設定ファイルを開く
settings-mode-help = システムに合わせて、ライトテーマとダークテーマを自動的に切り替えます。
settings-light-help = ライトモードで使用します。
settings-dark-help = ダークモードで使用します。
settings-pi-unsaved = パスは未保存です。確認すると現在の入力を検証しますが、保存はされません。
settings-pi-saved = 現在のパスは保存されています。Pi が利用できることをもう一度確認してください。
settings-resource-location = 場所を表示
settings-package-source = パッケージの取得元
settings-package-empty = 個人用の Pi パッケージはインストールされていません。
settings-resource-name = 名前
settings-resource-none = この種類の個人用リソースはまだありません。
settings-resource-view = 表示
settings-resource-readonly-badge = 読み取り専用
settings-source-personal = 個人
settings-source-external = 外部
settings-system-replace = 既定のシステムプロンプトを置き換える
settings-system-replace-help = Pi の既定のシステムプロンプトを完全に置き換えます。
settings-system-append = 個人用の指示を追加
settings-system-append-help = 既定のシステムプロンプトを維持し、その後に指示を追加します。
settings-resource-success = 「{ $target}」に対する操作が完了しました。既存のセッションを手動で再読み込みしてください。
settings-resource-failed = 「{ $target}」に対する操作に失敗しました。
settings-about-help = 保存済みパスの最新の確認結果を表示します。もう一度確認するには Pi の設定を開いてください。
settings-key-reveal-session = セッションファイルを表示

settings-key-group-app = アプリケーション
settings-key-group-conversation = 会話
settings-key-group-files = セッションファイル

settings-package-kind-extensions = 拡張機能
settings-package-kind-skills = スキル
settings-package-kind-themes = テーマ
settings-package-kind-prompts = テンプレート
settings-package-expand = パッケージの内容を展開
settings-package-collapse = パッケージの内容を折りたたむ

settings-template-search = テンプレート名、説明、取得元を検索
settings-template-collapse = プレビューを折りたたむ

temporary-title = 一時会話
attachment-add = ファイルを添付
attachment-remove = 添付を削除
attachment-file = ファイル
attachment-clipboard = クリップボードの画像
image-preview-close = プレビューを閉じる
image-preview-open = 画像のプレビューを開く
image-preview-zoom-in = 拡大
image-preview-zoom-out = 縮小
shortcut-global = グローバルショートカット
shortcut-launcher = 一時ウィンドウ
shortcut-add = テンプレートタスクを追加
shortcut-edit = テンプレートタスクを編集
shortcut-delete = テンプレートタスクを削除
shortcut-name = 名前
shortcut-template = コマンドテンプレート
shortcut-source = 入力元
shortcut-selection = 選択したテキスト
shortcut-clipboard = クリップボード
shortcut-fallback = 選択したテキスト。ない場合はクリップボード
shortcut-model = モデル
shortcut-thinking = 思考レベル
shortcut-default-model = Pi の既定モデル
shortcut-default-thinking = Pi の既定の思考レベル
shortcut-personal = 個人用
shortcut-template-required = コマンドテンプレートを選択してください。
shortcut-reload-options = オプションを再読み込み
temporary-clean-released = 解放済みのワークスペース
temporary-clean = クリーンアップ
temporary-clean-help = 解放済みの一時ワークスペースをゴミ箱に移動します。実行中の会話は保持されます。
temporary-cleaned = ワークスペースをゴミ箱に移動しました
shortcut-tasks = テンプレートタスク

shortcut-select-template = プロンプトテンプレートを選択

temporary-search-placeholder = 一時会話を検索
temporary-search-empty = 一致する一時会話はありません
temporary-toggle-input = 入力欄を切り替え
temporary-hide = 非表示
session-search-open = 会話を開く

temporary-actions = 操作
temporary-paste-answer = 直前の回答を貼り付け
temporary-reveal-workspace = 作業ディレクトリを表示
temporary-search-actions = 操作を検索…
temporary-switch-session = 一時会話を切り替え
temporary-paste-failed = 回答をコピーしました。自動貼り付けに失敗しました。アクセシビリティ権限と貼り付け先を確認するか、手動で貼り付けてください。
temporary-session-1 = 一時会話 1 に切り替え
temporary-session-2 = 一時会話 2 に切り替え
temporary-session-3 = 一時会話 3 に切り替え
temporary-session-4 = 一時会話 4 に切り替え
temporary-session-5 = 一時会話 5 に切り替え
temporary-session-6 = 一時会話 6 に切り替え
temporary-session-7 = 一時会話 7 に切り替え
temporary-session-8 = 一時会話 8 に切り替え
temporary-session-9 = 最後の一時会話に切り替え
temporary-send = 送信
temporary-trash = 一時会話をゴミ箱に移動
settings-key-stop-or-hide = 生成を停止 / 一時ウィンドウを非表示

conversation-queue-title = キューに追加済み（{ $count}）
conversation-queue-steer = 現在のターンに指示
conversation-queue-follow-up = 追加タスク
conversation-queue-restore = キュー内のテキストをすべて下書きに戻す
conversation-queue-clear = キュー内のメッセージをすべて消去
conversation-queue-no-text = テキストのないメッセージ
conversation-queue-unavailable = キューの内容はまだ利用できません
conversation-queue-text-only = キューから戻すとテキストのみ復元され、画像は復元されません。

conversation-retry-countdown = 約 { $seconds} 秒後に再試行（{ $attempt}/{ $total}）
conversation-retry-waiting = 再試行（{ $attempt}/{ $total}）：Pi を待っています
conversation-summary-retry-countdown = 概要を約 { $seconds} 秒後に再試行（{ $attempt}/{ $total}）
conversation-summary-retry-waiting = 概要を再試行（{ $attempt}/{ $total}）：Pi を待っています

message-details-open = 詳細を表示…
message-details-copy-all = すべてコピー
tool-detail-command = コマンド
tool-detail-content = 内容
tool-detail-changes = 変更

settings-notifications = 通知
settings-notification-help = システム通知には一般的な説明が表示されます。通知を開くと会話に戻ります。会話が前面に表示されると未読数が消去されます。
settings-notification-waiting = バックグラウンドで入力が必要なときに通知
settings-notification-failures = バックグラウンドでタスクが失敗したときに通知
settings-notification-plugins = プラグインのリマインダーをシステム通知で送信
settings-notification-attention = バックグラウンドで入力が必要なときに注意を促す
settings-notification-completion = 回答完了の通知
notification-off = オフ
notification-background = バックグラウンドのみ
notification-always = 常に表示
notification-waiting = 入力を待っています
notification-completed = 回答の準備ができました
notification-failed = タスクが失敗し、確認が必要です
notification-plugin = プラグインのリマインダー
notification-unread = 未読の会話
notification-running = 実行中
notification-clear = リマインダーを消去
notification-session-notices = 会話のリマインダー

settings-icon-theme = アプリアイコン
settings-icon-theme-help = アプリ内ロゴと、実行中アプリの macOS Dock アイコンを変更します。Finder と Windows タスクバーでは同梱アイコンが使われます。
icon-theme-classic = クラシック
icon-theme-classic-gradient = クラシック（グラデーション）
icon-theme-color = Pi カラー
icon-theme-color-gradient = Pi グラデーション
icon-theme-pride = レインボー
icon-theme-ukraine = ウクライナ
icon-theme-ukraine-gradient = ウクライナ（グラデーション）

language-traditional-chinese = 繁體中文
language-japanese = 日本語
language-korean = 한국어
language-german = Deutsch
language-french = Français
language-spanish = Español
language-portuguese-brazil = Português (Brasil)
menu-about = Gupi について
menu-conversation = 会話
menu-edit = 編集
menu-view = 表示
menu-window = ウィンドウ
menu-help = ヘルプ
menu-services = サービス
menu-hide = Gupi を隠す
menu-hide-others = ほかを隠す
menu-show-all = すべて表示
menu-undo = 取り消す
menu-redo = やり直す
menu-cut = 切り取り
menu-copy = コピー
menu-paste = 貼り付け
menu-select-all = すべて選択
menu-minimize = 最小化
menu-zoom = 拡大／縮小
menu-fullscreen = フルスクリーンを切り替え
menu-docs = Gupi ユーザーガイド
menu-pi-docs = Pi ドキュメント
menu-report-issue = 問題を報告
menu-logs = ログを表示
menu-copy-diagnostics = 診断情報をコピー
settings-permissions = システム権限
settings-accessibility-help = 選択したテキストの読み取りや別のアプリへの貼り付けには、アクセシビリティの許可が必要な場合があります。システム設定で Gupi を許可してから、もう一度操作してください。貼り付けに失敗しても回答はコピーできます。
settings-accessibility-open = アクセシビリティ設定
settings-notification-permission-help = 通知や Dock バッジが表示されない場合は、システム設定で Gupi の通知とバッジを許可してください。この機能を初めて使うときに許可を求めます。
settings-notification-permission-open = 通知設定
settings-native-language-help = アプリの言語はすぐに切り替わります。macOS のシステムダイアログに適用するには Gupi を再起動してください。Windows のシステムダイアログは Windows の表示言語に従います。
settings-diagnostics-help = アプリのバージョン、プラットフォーム、Pi コマンド、設定とログの保存場所が含まれます。会話、認証情報、環境変数は含まれません。

setup-preferences-title = 言語と外観
setup-preferences-step = 言語と外観
setup-desktop-title = ショートカットと通知
setup-desktop-step = ショートカットと通知
setup-desktop-help = これらの設定は後から設定で変更できます。
setup-page-skip = このページをスキップ
setup-skip-all = 既定値を使用
setup-pi-later = Pi を後から設定
action-open = 開く

setup-appearance-options = テーマとアイコンを選択
setup-pi-install-guide = Pi をインストール
setup-pi-model-guide = モデルに接続
