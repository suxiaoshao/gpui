app-title = AI Chat
settings-title = Settings
message-preview-title = Message Preview: { $id }
app-menu-about = About {$app_name}
app-menu-version = Version (v{ $version })
app-menu-open-main = Open AI Chat
app-menu-open-temporary = Open Temporary Conversation
app-menu-settings = Settings
app-menu-services = Services
app-menu-hide = Hide {$app_name}
app-menu-hide-others = Hide Others
app-menu-show-all = Show All
app-menu-quit = Quit {$app_name}
app-menu-window = Window
app-menu-minimize = Minimize
app-menu-zoom = Zoom
app-about-window-title = About {$app_name}
app-about-version = Version {$version}
app-about-github = GitHub
tray-open-main = Open AI Chat
tray-open-temporary = Open Temporary Conversation
tray-version = Version (v{ $version })
tray-about = About AI Chat
tray-quit = Quit AI Chat
tray-tooltip = AI Chat
tray-about-comments = A desktop AI chat app built with GPUI.
tray-about-website-label = Project Repository

settings-page-general = General
settings-page-provider = Provider
settings-page-shortcuts = Shortcuts
settings-group-basic-options = Basic Options
settings-group-shortcuts = Global Shortcut Bindings
language-system = System
language-english = English
language-chinese = 简体中文

sidebar-app-title = AI Chat
sidebar-conversation-tree = Conversation Tree
sidebar-actions = Actions
sidebar-settings = Settings
sidebar-search-conversation = Search Conversations
sidebar-template-list = Template List
sidebar-add-conversation = Add Conversation
sidebar-add-folder = Add Folder
sidebar-root = Root

tab-templates = Templates
tab-template = Template

section-information = Information
alert-error-title = Error
message-status-paused = Paused

button-add = Add
button-add-prompt = Add Prompt
dialog-edit-folder-title = Edit Folder
dialog-edit-conversation-title = Edit Conversation
notify-update-folder-failed = Update Folder Failed
notify-update-conversation-failed = Update Conversation Failed

button-configure = Config
button-cancel = Cancel
button-create = Create
button-delete = Delete
button-edit = Edit
button-export = Export
button-open = Open
button-copy-to-new-conversation = Copy to New Conversation
button-clear = Clear Conversation
button-reload = Reload
button-reset = Reset
button-save = Save Conversation
button-submit = Submit

tooltip-copy = Copy
tooltip-delete = Delete
tooltip-view-detail = View Detail
tooltip-resend-message = Resend Message
tooltip-send-message = Send Message
tooltip-pause-message = Pause Generation
tooltip-clear-conversation = Clear Conversation
tooltip-save-conversation = Save Conversation
tooltip-copy-conversation = Copy to New Conversation
tooltip-export-conversation = Export Conversation
tooltip-detach-temporary = Detach Temporary Conversation
tooltip-ollama-web-search-help =
    Ollama `web_search` depends on `web_fetch`.

    Ollama `web_search` / `web_fetch` uses cloud capabilities. It is not a local search-plugin switch.

    ## 1. Confirm your Ollama version supports the experimental routes

    ```bash
    ollama --version
    ```

    If the request below returns `404`, your current version does not expose this route yet and you need to upgrade Ollama first:

    ```bash
    curl http://localhost:11434/api/experimental/web_fetch \
      -H 'Content-Type: application/json' \
      -d '{"{"}"url":"https://ollama.com"{"}"}'
    ```

    ## 2. Sign in to Ollama Cloud

    ```bash
    ollama signin
    ```

    Local `localhost:11434` routes use your machine's sign-in state. API keys are only needed when calling `https://ollama.com/api/*` directly.

    ## 3. Confirm cloud is not disabled

    ```bash
    echo "$OLLAMA_NO_CLOUD"
    cat ~/.ollama/server.json
    ```

    Make sure:
    - `OLLAMA_NO_CLOUD` is not `1` / `true`
    - `~/.ollama/server.json` does not contain `"disable_ollama_cloud": true`

    ## 4. Restart Ollama

    Restart Ollama after changing sign-in state or cloud configuration.

    ## 5. Verify status

    ```bash
    curl http://localhost:11434/api/status
    ```

    Expected:
    - `cloud.disabled` is `false`

    Then verify the experimental route:

    ```bash
    curl http://localhost:11434/api/experimental/web_fetch \
      -H 'Content-Type: application/json' \
      -d '{"{"}"url":"https://ollama.com"{"}"}'
    ```

    Result guide:
    - `404`: your Ollama version does not support the route yet, upgrade first
    - `401`: not signed in, run `ollama signin` again
    - `403`: cloud is disabled, check `OLLAMA_NO_CLOUD` and `server.json`
    - `200`: ready to use

    Official docs:
    - [Web search](https://ollama.com/blog/web-search)
    - [API docs](https://ollama.com/api)
temporary-chat-title = Temporary Chat
temporary-chat-description = Start a temporary conversation and choose a template from the chat form when needed.

field-id = ID
field-name = Name
field-icon = Icon
field-info = Info
field-theme = Theme
field-language = Language
field-config-file = Config File
field-http-proxy = Http Proxy
field-temporary-conversation-hotkey = Temporary Conversation Hotkey
field-template = Template
field-template-prefix = Template
field-provider = Provider
field-conversation-name = Conversation Name
field-conversation-path = Conversation Path
field-mode = Mode
field-description = Description
field-enabled = Enabled
field-none = None
field-on = On
field-off = Off
field-search-extension = Search extensions
field-search-conversation = Search conversations
field-search-template = Search templates
field-search-models = Search models
mode-contextual = Contextual
mode-single = Single
mode-assistant-only = Assistant Only
field-prompts = Prompts
field-role = Role
field-prompt = Prompt
field-chat-input-placeholder = Ask anything
field-model = Model
field-preset = Preset
field-hotkey = Hotkey
field-thinking = Thinking
field-reasoning-effort = Reasoning Effort
field-reasoning-summary = Reasoning Summary
field-extension = Extension
field-loading = Loading...
field-models = Model Select
field-web-search = Web Search
field-api-key = Api Key
field-base-url = Base URL
field-sources = Sources
field-created-time = Created Time
field-updated-time = Updated Time
field-start-time = Start Time
field-end-time = End Time
field-status = Status
field-error = Error
field-text = Text
field-citations = Citations
field-send-content = Send Content
field-actions = Actions

dialog-add-folder-title = Add Folder
dialog-add-conversation-title = Add Conversation
dialog-search-conversation-title = Search Conversations
dialog-edit-template-title = Edit Template
dialog-add-template-title = Add Template
dialog-delete-template-title = Delete Template
dialog-delete-template-message = Delete this template? This action cannot be undone.
dialog-delete-shortcut-title = Delete Shortcut
dialog-delete-shortcut-message = Delete this shortcut binding? This action cannot be undone.

notify-get-templates-failed = Get Templates Failed
notify-select-template = Please select a template
notify-add-conversation-failed = Add Conversation Failed
notify-add-folder-failed = Add Folder Failed
notify-move-conversation-failed = Move Conversation Failed
notify-move-folder-failed = Move Folder Failed
notify-delete-message-failed = Delete Message Failed
notify-delete-conversation-failed = Delete Conversation Failed
notify-delete-folder-failed = Delete Folder Failed
notify-delete-template-failed = Delete template failed
notify-template-deleted-success = Template deleted successfully
notify-load-template-failed = Load template failed
notify-load-templates-failed = Load templates failed
notify-load-models-partial-failed = Some model providers failed to load
notify-load-shortcuts-failed = Load shortcut bindings failed
notify-load-template-schema-failed = Load template schema failed
notify-open-database-failed = Open database failed
notify-reload-template-failed = Reload template failed
notify-select-model = Please select a model
notify-select-mode = Please select a mode
notify-select-adapter = Please select an adapter
notify-invalid-template = Invalid template
notify-invalid-prompts = Invalid prompts
notify-invalid-shortcut-name = Shortcut name cannot be empty
notify-invalid-shortcut-hotkey = Please record a valid hotkey
notify-no-adapter-available = No adapter available
notify-shortcut-created-success = Shortcut binding created successfully
notify-shortcut-updated-success = Shortcut binding updated successfully
notify-shortcut-deleted-success = Shortcut binding deleted successfully
notify-shortcut-trigger-busy-title = Shortcut Busy
notify-shortcut-trigger-busy-message = The temporary window is still processing another request.
notify-shortcut-trigger-empty-input-title = No Input Content
notify-shortcut-trigger-empty-input-message = No selected text or clipboard text is available.
notify-shortcut-trigger-model-unavailable-title = Shortcut Model Unavailable
notify-shortcut-trigger-screenshot-title = Screenshot Capture Failed
notify-shortcut-trigger-ocr-title = Screenshot OCR Failed
notify-save-shortcut-failed = Save shortcut binding failed
notify-delete-shortcut-failed = Delete shortcut binding failed
notify-template-created-success = Template created successfully
notify-template-updated-success = Template updated successfully
notify-create-template-failed = Create template failed
notify-update-template-failed = Update template failed
notify-update-message-success = Update Message Content Success
notify-update-message-failed = Update Message Content Failed
notify-copy-success-title = Copy Succeeded
notify-copy-success-message = Message copied to clipboard.
notify-copy-failed-title = Copy Failed
notify-copy-failed-message = Could not read clipboard.
notify-open-config-file-failed = Open config file failed
notify-export-conversation-success = Conversation exported
notify-export-conversation-failed = Export conversation failed
conversation-search-no-results = No conversations found.

delete-message-failed-title = Delete Message Failed
delete-message-failed-message = Message view not available.

settings-openai-title = OpenAI
settings-ollama-title = Ollama

reasoning-effort-none = None
reasoning-effort-minimal = Minimal
reasoning-effort-low = Low
reasoning-effort-medium = Medium
reasoning-effort-high = High
reasoning-effort-xhigh = X-High
button-reasoning-summary = Thought
button-reasoning-summary-thinking = Thinking

template-error-select-role = Please select role for prompt
template-error-prompt-empty = Prompt cannot be empty:

field-raw = Raw
empty-model-picker = No models available
empty-template-picker = No templates available
empty-shortcut-bindings = No shortcut bindings yet
shortcut-ext-settings-unavailable = Preset options are unavailable until the current model can be resolved.
shortcut-model-unavailable = unavailable
send-content-selection-or-clipboard = Selection / Clipboard
send-content-screenshot = Screenshot
