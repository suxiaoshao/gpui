app-title = Gupi
menu-settings = Settings
menu-show-main = Main window
menu-quit = Quit Gupi
startup-welcome = Welcome to Gupi
temporary-setup-required = Complete setup in Settings to start a temporary conversation.
startup-checking = Checking…
startup-quitting = Finishing operations…
settings-pi-command = Pi executable
settings-path-help = Leave empty to find pi automatically, or enter an absolute executable path without arguments.
settings-theme = Appearance
settings-language = Language
settings-save-pi = Save Pi path
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
recovery-confirm = Continue? Reload discards your draft only after success. Reset first backs up the current file.
recovery-backup = Backup
error-config-read = Could not read the configuration. Check its location and permissions, then reload.
error-config-parse = Configuration is not valid. Reload a corrected file, or back it up and reset.
error-config-validation = Enter one executable name or an absolute executable path, without arguments.
error-config-write = Saving failed. Your draft and applied settings are retained. Check the configuration file and try again.
error-pi-probe = Pi could not be verified. Check the executable path and its Node environment, then retry.
action-check-pi = Check Pi again
action-reset = Back up and reset
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
conversation-restoring = Restoring drafts…
conversation-discovering = Finding conversations… { $count } files
conversation-read-progress = Read { $completed } / { $total } files
conversation-refresh-progress = Refreshing { $completed } / { $total } files
conversation-project = Choose working directory
conversation-untitled = Untitled conversation
conversation-idle = Idle
conversation-loading = Opening session
conversation-running = Running
conversation-failed = Error needs attention
conversation-waiting = Waiting for input
conversation-search = Search conversations
conversation-refresh = Refresh sessions
conversation-refresh-current = Refresh current session
conversation-actions = Session actions
conversation-show-sidebar = Show session sidebar
conversation-hide-sidebar = Hide session sidebar
conversation-scan-failed = Could not load the conversation catalog
conversation-search-placeholder = Search names, messages, or project paths
conversation-search-empty = No matching conversations
conversation-rename = Rename…
conversation-delete = Move to Trash
conversation-delete-failed = Could not delete conversation: { $error }
conversation-copy-path = Copy path
conversation-clone = Duplicate conversation
conversation-export = Export conversation as HTML…
conversation-exported = Conversation exported to { $path }
conversation-stop = Stop generation
conversation-close-run = Close runtime
conversation-show-less = Show less
conversation-show-more = Show more…
conversation-fork = Fork conversation from here
conversation-preview = Previewing another branch
conversation-return-current = Return to current branch
conversation-welcome = Start a conversation
conversation-welcome-project = What would you like to do in { $project }?
conversation-empty-history = No messages in this conversation yet.
conversation-bottom = Jump to bottom
conversation-copy = Copy
conversation-compaction = Context compaction summary
conversation-branch-summary = Branch summary
conversation-working = Working…
conversation-process = View process
conversation-details = Toggle details
tool-detail-offset = Starting line:
tool-detail-line-limit = Requested line limit:
tool-detail-timeout = Timeout (seconds):
tool-detail-output = Output
tool-detail-input = Input
tool-detail-additional = Additional information
tool-detail-error = Error
tool-detail-glob = File pattern:
tool-detail-ignore-case = Ignore case:
tool-detail-literal = Literal search:
tool-detail-context = Context lines:
tool-detail-result-limit = Requested result limit:
tool-detail-old-text = Requested original text
tool-detail-new-text = Requested replacement text
tool-detail-truncated = Pi truncated this output; the content shown is not the full result.
tool-detail-lines-truncated = Pi shortened some result lines.
tool-detail-match-limit = Match limit reached:
tool-detail-results-limited = Result limit reached:
tool-detail-entries-limited = Directory entry limit reached:
tool-detail-full-output = Full output file:
tool-detail-image-unavailable = Unable to display this image.
conversation-role-user = User message
conversation-role-assistant = Assistant message
conversation-role-tool = Tool result
conversation-event = Session event
conversation-close-history = Close conversation history
conversation-source = Reveal source session file
conversation-reconnect = Reconnect
conversation-save-error = Draft could not be saved
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
conversation-sending = Sending…
conversation-graph-current = Current
conversation-graph-preview = Preview
conversation-graph-left = Show lanes to the left (or scroll horizontally)
conversation-graph-right = Show lanes to the right (or scroll horizontally)
conversation-graph-reveal = Reveal selected
conversation-graph-column = Graph
conversation-graph-message = Message
conversation-catalog-empty = No conversations yet

conversation-thinking-content = Thinking
conversation-thinking-running = Thinking…
conversation-tool-group = Tool calls ({ $count })
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
conversation-history-brief = Brief: user messages and final answers
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
composer-model-loading = Loading models…
composer-thinking-loading = Loading thinking levels…
composer-model-confirming = Confirming model settings…
composer-stats-loading = Loading usage…
composer-stats-retry = Retry loading usage
conversation-checking-file = Checking conversation file…
conversation-connecting = Connecting to Pi…
conversation-history-loading = Loading conversation history…
conversation-history-refreshing = Refreshing conversation history…
conversation-fork-options-loading = Loading fork options…
conversation-fork-options-retry = Retry loading fork options
history-canvas-zoom-in = Zoom in
history-canvas-zoom-out = Zoom out
history-canvas-fit = Fit tree (0)
history-canvas-current = Locate execution node
history-canvas-expand = Expand { $count } nodes
history-canvas-collapse = Collapse { $count } process nodes, keeping navigation markers
history-canvas-empty = No history nodes to display
history-canvas-help = Conversation tree: scroll or drag to pan, pinch or plus/minus to zoom, arrow keys to select, Enter to preview, E to expand the next segment

history-view-tree = Tree
history-view-list = List
history-level-brief = Brief
history-level-detailed = Detailed
history-level-all = All
history-level-all-description = All: including settings, labels and custom records
history-scope-all = All branches
history-scope-branch = This branch only
history-model-change = Model change
history-thinking-change = Thinking level change
history-session-info = Session information change
history-label-change = Label change
history-custom-record = Custom record


history-content = Content
history-range = Scope

command-palette = Command Palette
command-focus-input = Focus Conversation Input
command-model = Select Model and Thinking Level…
command-copy-last-answer = Copy Last Answer
command-compact = Compact Context
conversation-compacting = Compacting context…
command-history-description = View history or choose a user message to fork
command-show-history = Show History
command-hide-history = Hide History
command-current-session = Current session:
command-unavailable = Unavailable in the current state
command-target-changed = This conversation is no longer available. Select another conversation.
command-scanning = Loading conversations…
command-loading = Loading Pi commands…
command-no-matches = No matching Pi commands
conversation-reconnect-unconfirmed = Pi process exit could not be confirmed. Restart Gupi before reconnecting.
action-retry = Retry

command-search-placeholder = Search actions or enter a command
command-empty = No Pi commands available in this session

command-group-app = Application
command-group-extensions = Extension commands
command-group-skills = Skills
command-group-prompts = Prompt templates
command-scope-user = Personal
command-scope-project = Project
command-scope-temporary = Temporary
command-connection-unavailable = Pi is not ready
command-dismiss = Close
command-complete = Complete
command-send-text = Send
command-execute = Run

command-scope-current = Current session

conversation-working-duration = Working { $duration }
conversation-tool-group-skill = Skills read ({ $count })
conversation-shell-line = { $shell } · { $action } { $summary }
conversation-tool-action-skill =
    { $state ->
        [running] Reading skill
        [complete] Read skill
        [failed] Failed to read skill
       *[unfinished] Skill read unfinished
    }

# Unified settings
settings-page-general = General
settings-page-pi = Pi
settings-page-keys = Keyboard shortcuts
settings-page-plugins = Plugins
settings-page-skills = Skills
settings-page-prompts = Prompts
settings-page-about = About
settings-key-reset = Restore default
settings-key-reset-all = Restore all defaults
settings-key-reset-all-confirm = Restore the default bindings for all shortcuts? All custom shortcuts will be cleared.
settings-key-clear = Clear shortcut
settings-key-cancel = Cancel changes
settings-key-unbound = No shortcut assigned
settings-key-record = Record
settings-key-recording = Press a shortcut…
settings-key-conflict = Conflicting or invalid shortcut
settings-key-invalid = Enter a valid shortcut, or leave empty to unbind
settings-resource-refresh = Refresh resources
settings-resource-reload-help = Personal resources only. Reload existing sessions manually to apply changes.
settings-resource-working = Working…
settings-resource-pi-required = Save and check a working Pi executable in Pi settings first.
settings-resource-invalid-name = Names may contain letters, digits, hyphens and underscores.
settings-resource-register = Add local
settings-resource-create = Create
settings-resource-name-help = New resource name: letters, digits, hyphens or underscores.
settings-resource-empty = No matching resources.
settings-resource-delete = Move to trash
settings-resource-save = Save text
shortcut-save-task = Save
settings-resource-loading = Reading text…
settings-resource-confirm-remove = Remove this item? File deletion moves only the selected file to trash, keeping its directory.
settings-package-install = Install package
settings-package-update = Update
settings-package-remove = Remove
settings-package-source-help = Package source: npm:name, a Git URL, or an absolute local path.
settings-package-extensions = Extensions { $count }
settings-package-skills = Skills { $count }
settings-package-prompts = Templates { $count }
settings-package-themes = Themes { $count }
settings-skill-search = Search skills by name, description, path or source
settings-skill-collapse = Collapse
settings-resource-edit = Edit
settings-about-gupi = Version
settings-about-pi = Version
settings-about-path = Resolved path
settings-about-status = Probe status
settings-about-unavailable = Not available yet
settings-editor-discard = Discard unsaved text changes?
settings-package-confirm-remove = Remove this package from personal Pi settings? Local package source directories are retained.
settings-package-heading = Installed packages
settings-extension-heading = Standalone extensions
settings-template-heading = Command templates
settings-system-heading = System prompts
settings-config-heading = Configuration file
settings-config-open = Open configuration file
settings-mode-help = Follow the system to switch between your light and dark themes automatically.
settings-light-help = Used when light mode is active.
settings-dark-help = Used when dark mode is active.
settings-pi-unsaved = Unsaved path. Checking validates the current input without saving it.
settings-pi-saved = The current path is saved. Check again to verify Pi is available.
settings-resource-location = Show location
settings-package-source = Package source
settings-package-empty = No personal Pi packages installed.
settings-resource-name = Name
settings-resource-none = No personal resources of this type yet.
settings-resource-view = View
settings-resource-readonly-badge = Read-only
settings-source-personal = Personal
settings-source-external = External
settings-system-replace = Replace the default system prompt
settings-system-replace-help = Completely replaces Pi’s default system prompt.
settings-system-append = Append personal instructions
settings-system-append-help = Keeps the default system prompt and appends these instructions.
settings-resource-success = Completed the operation on “{ $target }”. Reload existing sessions manually.
settings-resource-failed = The operation on “{ $target }” failed.
settings-about-help = Shows the latest check of the saved path. Open Pi settings to check again.
settings-key-reveal-session = Show session file

settings-key-group-app = Application
settings-key-group-conversation = Conversation
settings-key-group-files = Session files

settings-package-kind-extensions = Extensions
settings-package-kind-skills = Skills
settings-package-kind-themes = Themes
settings-package-kind-prompts = Templates
settings-package-expand = Expand package contents
settings-package-collapse = Collapse package contents

settings-template-search = Search template names, descriptions, or sources
settings-template-collapse = Collapse preview

temporary-title = Temporary conversations
attachment-add = Attach files
attachment-remove = Remove attachment
attachment-file = File
attachment-clipboard = Clipboard image
image-preview-close = Close preview
image-preview-open = Open image preview
image-preview-zoom-in = Zoom in
image-preview-zoom-out = Zoom out
shortcut-global = Global shortcuts
shortcut-launcher = Temporary window
shortcut-add = Add template task
shortcut-edit = Edit template task
shortcut-delete = Delete template task
shortcut-name = Name
shortcut-template = Command template
shortcut-source = Input source
shortcut-selection = Selected text
shortcut-clipboard = Clipboard
shortcut-fallback = Selected text, otherwise clipboard
shortcut-model = Model
shortcut-thinking = Thinking level
shortcut-default-model = Pi default model
shortcut-default-thinking = Pi default thinking level
shortcut-personal = Personal
shortcut-template-required = Select a command template.
shortcut-reload-options = Reload options
temporary-clean-released = Released workspaces
temporary-clean = Clean up
temporary-clean-help = Move released temporary workspaces to Trash. Active conversations are kept.
temporary-cleaned = Workspaces moved to Trash
shortcut-tasks = Template tasks

shortcut-select-template = Choose a prompt template

temporary-search-placeholder = Search temporary conversations
temporary-search-empty = No matching temporary conversations
temporary-toggle-input = Switch input
temporary-hide = Hide
session-search-open = Open conversation

temporary-actions = Actions
temporary-paste-answer = Paste Last Answer
temporary-reveal-workspace = Show Working Directory
temporary-search-actions = Search for actions…
temporary-switch-session = Switch Temporary Conversation
temporary-paste-failed = Answer copied. Automatic paste failed. Check Accessibility permission and the target application, or paste manually.
temporary-session-1 = Switch to Temporary Conversation 1
temporary-session-2 = Switch to Temporary Conversation 2
temporary-session-3 = Switch to Temporary Conversation 3
temporary-session-4 = Switch to Temporary Conversation 4
temporary-session-5 = Switch to Temporary Conversation 5
temporary-session-6 = Switch to Temporary Conversation 6
temporary-session-7 = Switch to Temporary Conversation 7
temporary-session-8 = Switch to Temporary Conversation 8
temporary-session-9 = Switch to Last Temporary Conversation
temporary-send = Send
temporary-trash = Move Temporary Conversation to Trash
settings-key-stop-or-hide = Stop Generation / Hide Temporary Window

conversation-queue-title = Queued ({ $count })
conversation-queue-steer = Steer current turn
conversation-queue-follow-up = Follow-up task
conversation-queue-restore = Return all queued text to draft
conversation-queue-clear = Clear all queued messages
conversation-queue-no-text = Message without text
conversation-queue-unavailable = Queue content is not available yet
conversation-queue-text-only = Returning the queue restores text only; queued images cannot be restored.

conversation-retry-countdown = Retry { $attempt }/{ $total } in about { $seconds }s
conversation-retry-waiting = Retry { $attempt }/{ $total }: waiting for Pi
conversation-summary-retry-countdown = Summary retry { $attempt }/{ $total } in about { $seconds }s
conversation-summary-retry-waiting = Summary retry { $attempt }/{ $total }: waiting for Pi
