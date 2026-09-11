app-title = Gupi
menu-settings = Settings
menu-show-main = Main window
menu-quit = Quit Gupi
startup-welcome = Welcome to Gupi
startup-checking = Checking…
startup-quitting = Finishing operations…
settings-pi-command = Pi executable
settings-path-help = Leave empty to find pi automatically, or enter an absolute executable path without arguments.
settings-theme = Appearance
settings-language = Language
settings-save = Save settings
settings-reload = Reload from disk
settings-write-current = Write applied settings to disk (keep draft)
theme-system = System
theme-light = Light
theme-dark = Dark
language-system = System
language-english = English
language-chinese = 简体中文
recovery-config-title = Configuration needs attention
recovery-pi-title = Set up Pi
recovery-confirm = Continue? Reload discards your draft only after success. Reset or overwrite first backs up the current file.
recovery-backup = Backup
error-config-read = Could not read the configuration. Check its location and permissions, then reload.
error-config-parse = Configuration is not valid. Reload a corrected file, or back it up and reset.
error-config-validation = Enter one executable name or an absolute executable path, without arguments.
error-config-conflict = The file changed externally. Reload it, or explicitly back it up and overwrite with the submitted settings.
error-config-write = Saving failed. Your draft and applied settings are retained. Reload to reconcile uncertain commits.
error-pi-probe = Pi could not be verified. Check the executable path and its Node environment, then retry.
action-check-pi = Check Pi again
action-reset = Back up and reset
action-overwrite = Back up and overwrite
action-confirm = Confirm
action-cancel = Cancel
home-ready = Pi is available
home-description = This version provides environment setup and desktop preferences. Conversation support is planned for the next stage.
action-locate = Show configuration folder
error-pi-timeout = Pi did not finish the version check within 15 seconds.
error-pi-output = Pi returned more output than a version check permits.
error-pi-version = Pi exited unsuccessfully or did not return a valid version.
error-log = File logging is unavailable. Check the log directory and permissions.

setup-tagline = Your native desktop home for Pi
setup-intro = A quick setup to get started.
setup-start = Get started
setup-language-title = Feel at home
setup-search-language = Search languages…
setup-system-chinese = System language: 简体中文
setup-system-english = System language: English
setup-appearance-title = Make it yours
setup-color-mode = Color mode
setup-system-accent = System accent
setup-pi-title = Connect Pi
setup-pi-step = Pi setup
setup-pi-help = Connect your local Pi installation.
setup-check-pi = Check Pi
setup-back = Back
setup-next = Continue
setup-finish = Finish setup
setup-saving = Saving…
light-themes = Light themes
dark-themes = Dark themes

# Conversation workspace
conversation-new = New conversation
conversation-sidebar = Conversation sidebar
conversation-history = Conversation history
conversation-scanning = Loading conversations…
conversation-project = Choose working directory
conversation-untitled = Untitled conversation
conversation-idle = Idle
conversation-loading = Opening session
conversation-running = Running
conversation-failed = Error needs attention
conversation-waiting = Waiting for input
conversation-search = Search conversations
conversation-refresh = Refresh sessions
conversation-scan-warning = Some sessions could not be read
conversation-search-placeholder = Search names, messages, or project paths
conversation-search-empty = No matching conversations
conversation-rename = Rename…
conversation-copy-path = Copy path
conversation-stop = Stop generation
conversation-close-run = Close runtime
conversation-show-less = Show less
conversation-show-more = Show more…
conversation-fork = Fork conversation from here
conversation-preview = Previewing another branch
conversation-return-current = Return to current branch
conversation-welcome = Start a conversation
conversation-welcome-hint = Choose a working directory and write a message.
conversation-bottom = Jump to bottom
conversation-copy = Copy
conversation-compaction = Context compaction summary
conversation-branch-summary = Branch summary
conversation-archive = History before compaction
conversation-working = Working…
conversation-process = View process
conversation-details = Toggle details
conversation-role-user = User message
conversation-role-assistant = Assistant message
conversation-role-tool = Tool result
conversation-event = Session event
conversation-close-history = Close conversation history
conversation-source = Reveal source session file
conversation-reconnect = Reconnect
conversation-save-error = Draft could not be saved
conversation-recover-draft = Restore unsent input
conversation-interrupted = Run stopped; existing content is preserved.
conversation-no = No
conversation-submit = Submit
conversation-input = Write a message
conversation-model = Model
conversation-load-options = Connect to Pi to load models
conversation-thinking = Thinking level
conversation-unknown = Unavailable
conversation-context = Current execution branch context
conversation-auto-compaction = Auto compaction
conversation-on = On
conversation-off = Off
conversation-tokens = Total input / output
conversation-cache = Total cache read / write
conversation-cache-hit = Latest cache hit rate
conversation-cost = Cost reported by Pi
conversation-statistics = Usage statistics
conversation-send = Send (Enter); Alt+Enter queues a follow-up
conversation-queued = Queued for Pi
conversation-accepted = Accepted by Pi
conversation-graph-current = Current
conversation-graph-preview = Preview
conversation-graph-left = Show lanes to the left (or scroll horizontally)
conversation-graph-right = Show lanes to the right (or scroll horizontally)
conversation-graph-reveal = Reveal selected
conversation-graph-column = Graph
conversation-graph-message = Message
conversation-refreshing = Refreshing conversations…
conversation-catalog-empty = No conversations yet

conversation-thinking-content = Thinking
conversation-tool-group = Tool calls ({ $count })
conversation-tool-group-running = Working on { $count } tool calls
conversation-tool-group-read = Read files ({ $count })
conversation-tool-group-bash = Commands ({ $count })
conversation-tool-group-search = Searches ({ $count })
conversation-tool-group-edit = File changes ({ $count })
conversation-tool-line = { $action } { $summary }
conversation-tool-action-read =
    { $state ->
        [running] Reading
        [complete] Read
        [failed] Failed to read
        *[unfinished] Unfinished read
    }
conversation-tool-action-write =
    { $state ->
        [running] Writing
        [complete] Wrote
        [failed] Failed to write
        *[unfinished] Unfinished write
    }
conversation-tool-action-edit =
    { $state ->
        [running] Editing
        [complete] Edited
        [failed] Failed to edit
        *[unfinished] Unfinished edit
    }
conversation-tool-action-bash =
    { $state ->
        [running] Running
        [complete] Ran
        [failed] Command failed
        *[unfinished] Unfinished command
    }
conversation-tool-action-search =
    { $state ->
        [running] Searching
        [complete] Searched
        [failed] Search failed
        *[unfinished] Unfinished search
    }
conversation-tool-action-other =
    { $state ->
        [running] Calling { $name }
        [complete] Called { $name }
        [failed] Failed call to { $name }
        *[unfinished] Unfinished call to { $name }
    }
conversation-tool-group-explore = Searched and read files
conversation-tool-group-explore-commands = Read files and ran commands

conversation-processed = Worked for { $duration }
conversation-processed-failed = Failed after { $duration }
conversation-processed-stopped = Stopped after { $duration }
conversation-copied = Copied
conversation-copy-failed = Copy failed. Please try again.
conversation-usage-title = Request usage
conversation-usage-model = Model
conversation-usage-provider = Provider
conversation-usage-input = Input tokens
conversation-usage-output = Output tokens
conversation-usage-cache-read = Cache read tokens
conversation-usage-cache-write = Cache write tokens
conversation-usage-total = Total tokens
conversation-usage-cost = Cost

composer-context-used = Used tokens
composer-context-limit = Context capacity
composer-context-percent = Context used
composer-token-input = Total input tokens
composer-token-output = Total output tokens
composer-token-cache-read = Total cache read tokens
composer-token-cache-write = Total cache write tokens
composer-token-usage = Session token usage

conversation-model-search = Search models…
conversation-model-empty = No matching models
conversation-model-settings-error = Unable to read model settings
conversation-thinking-off = Off
conversation-thinking-minimal = Minimal
conversation-thinking-low = Low
conversation-thinking-medium = Medium
conversation-thinking-high = High
conversation-thinking-xhigh = Extra High
conversation-thinking-max = Max

conversation-model-reasoning = Reasoning
conversation-model-vision = Vision

conversation-history-reply = Assistant reply
conversation-history-brief = Brief: user and assistant messages
conversation-history-detailed = Detailed: messages, tools and summaries
conversation-history-progress = Assistant progress
conversation-history-thinking = Thinking
conversation-history-calls = Tool calls
conversation-history-failed = Execution failed
conversation-history-stopped = Interrupted
conversation-history-empty = Reply without text
composer-model-thinking = Model and thinking
composer-model-refresh = Refresh models and thinking levels
composer-thinking-unavailable = Not supported
history-canvas-mode = Canvas: zoomable conversation tree
history-canvas-zoom-in = Zoom in
history-canvas-zoom-out = Zoom out
history-canvas-fit = Fit tree (0)
history-canvas-current = Locate execution node
history-canvas-expand = Expand { $count } nodes
history-canvas-collapse = Collapse this segment ({ $count } nodes)
history-canvas-empty = No history nodes to display
history-canvas-help = Conversation tree: scroll or drag to pan, pinch or plus/minus to zoom, arrow keys to select, Enter to preview, E to expand the next segment
