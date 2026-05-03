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
settings-page-appearance = Appearance
settings-page-provider = Provider
settings-page-templates = Templates
settings-page-shortcuts = Shortcuts
settings-group-basic-options = Basic Options
settings-group-shortcuts = Global Shortcut Bindings
settings-templates-description = Manage reusable prompt templates in one settings page.
settings-appearance-mode = Appearance Mode
settings-custom-theme-color = Custom Theme Color
settings-custom-theme-color-description = Pick a color to generate a Material You theme and add it to the theme pool.
settings-light-themes = Light Themes
settings-dark-themes = Dark Themes
theme-selected = Selected
theme-selected-prefix = Selected:
appearance-mode-system = System
appearance-mode-light = Light
appearance-mode-dark = Dark
language-system = System
language-english = English
language-chinese = 简体中文

sidebar-app-title = AI Chat
sidebar-conversation-tree = Conversation Tree
sidebar-actions = Actions
sidebar-settings = Settings
sidebar-search-conversation = Search Conversations
sidebar-add-conversation = Add Conversation
sidebar-add-folder = Add Folder
sidebar-root = Root

section-information = Information
section-content = Content
alert-error-title = Error
message-status-normal = Normal
message-status-hidden = Hidden
message-status-loading = Loading
message-status-thinking = Thinking
message-status-paused = Paused
message-status-error = Error
role-developer = Developer
role-user = User
role-assistant = Assistant

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
button-delete-template = Delete Template
button-edit = Edit
button-export = Export
button-view = View
button-close = Close
button-open = Open
button-copy-to-new-conversation = Copy to New Conversation
button-clear = Clear Conversation
button-reload = Reload
button-preview = Preview
button-reset = Reset
button-regenerate = Regenerate
button-save = Save Conversation
button-save-changes = Save Changes
button-save-message = Save Message
button-save-shortcut = Save Shortcut
button-submit = Submit
button-add-material-theme = Add Material You Theme
button-delete-material-theme = Delete Material You Theme

tooltip-copy = Copy
tooltip-delete = Delete
tooltip-view-detail = View Detail
tooltip-resend-message = Resend Message
tooltip-send-message = Send Message
tooltip-pause-message = Pause Generation
tooltip-clear-conversation = Clear Conversation
tooltip-save-conversation = Save Conversation
tooltip-select-template = Select Template
tooltip-show-api-key = Show API Key
tooltip-hide-api-key = Hide API Key
tooltip-copy-conversation = Copy to New Conversation
tooltip-export-conversation = Export Conversation
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
temporary-chat-empty-title = Start a temporary conversation
temporary-chat-empty-description = Choose a template or ask a question to begin.

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
field-search-settings = Search settings
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
field-api-key = API Key
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
dialog-view-template-title = Template
dialog-edit-template-title = Edit Template
dialog-add-template-title = Add Template
dialog-delete-conversation-title = Delete Conversation
dialog-delete-conversation-message = Delete conversation "{ $title }"? This action cannot be undone.
dialog-delete-folder-title = Delete Folder
dialog-delete-folder-message = Delete folder "{ $name }" and its contents? This action cannot be undone.
dialog-delete-message-title = Delete Message
dialog-delete-message-message = Delete this message? This action cannot be undone.
dialog-clear-conversation-title = Clear Conversation
dialog-clear-conversation-message = Clear all messages in this conversation? This action cannot be undone.
dialog-delete-temporary-message-title = Delete Temporary Message
dialog-delete-temporary-message-message = Delete this temporary message? This action cannot be undone.
dialog-clear-temporary-chat-title = Clear Temporary Chat
dialog-clear-temporary-chat-message = Clear all messages in this temporary chat? This action cannot be undone.
dialog-delete-material-theme-title = Delete Material You Theme
dialog-delete-material-theme-message = Delete this custom Material You theme? This action cannot be undone.
dialog-regenerate-message-title = Regenerate Message
dialog-regenerate-message-message = Regenerate this assistant message? Existing content will be overwritten.
dialog-delete-template-title = Delete Template
dialog-delete-template-message = Delete this template? This action cannot be undone.
dialog-add-shortcut-title = Add Shortcut
dialog-edit-shortcut-title = Edit Shortcut
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
notify-shortcut-reregistered-success = Shortcut registered again
notify-shortcut-reregister-failed = Re-register shortcut failed
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
empty-template-search = No templates match the search
conversation-search-no-results = No conversations found.
settings-search-no-results = No matching settings.

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
template-error-name-icon-required = Name and icon are required

field-raw = Raw
empty-model-picker = No models available
empty-template-picker = No templates available
empty-shortcut-bindings = No shortcut bindings yet
shortcut-empty-search = No shortcut bindings match the search
shortcut-search-placeholder = Search shortcuts or template names
shortcut-settings-description = Manage global shortcuts that send selected text, clipboard text, or screenshot OCR to a temporary conversation.
shortcut-filter-all = All
shortcut-filter-enabled = Enabled
shortcut-filter-disabled = Disabled
shortcut-filter-needs-action = Needs Action
shortcut-filter-all-modes = All Modes
shortcut-status-enabled = Enabled
shortcut-status-disabled = Disabled
shortcut-status-model-unavailable = Model Unavailable
shortcut-status-hotkey-invalid = Invalid Hotkey
shortcut-status-hotkey-conflict = Hotkey Conflict
shortcut-status-registration-failed = Registration Failed
shortcut-status-message-enabled = Shortcut is ready and waiting to be triggered.
shortcut-status-message-disabled = This shortcut is disabled.
shortcut-status-message-model-unavailable = The current model is not present in the model snapshot.
shortcut-status-message-not-registered = This shortcut is not registered with the system hotkey service.
shortcut-validation-temporary-conflict = Conflicts with the temporary conversation hotkey
shortcut-validation-binding-conflict = Conflicts with shortcut binding ID:
shortcut-registration-registered = Registered
shortcut-registration-not-registered = Not registered
shortcut-registration-disabled = Disabled, not registered
shortcut-runtime-waiting = Waiting
shortcut-runtime-screenshot-active = Screenshot selection active
shortcut-status-dialog-title = Shortcut Status
shortcut-status-message = Status Message
shortcut-status-registration = Hotkey Registration
shortcut-status-runtime = Runtime State
shortcut-action-reload-models = Reload Models
shortcut-action-reregister = Re-register Shortcut
shortcut-unsaved-changes = Unsaved changes
shortcut-hotkey-placeholder = Press a shortcut
shortcut-preset-settings = Preset / Extension Settings
shortcut-ext-settings-unavailable = Preset options are unavailable until the current model can be resolved.
shortcut-model-unavailable = unavailable
shortcut-mode-contextual-description = Include conversation context
shortcut-mode-single-description = Send only this request
shortcut-mode-assistant-only-description = Include assistant history only
send-content-selection-or-clipboard = Selection / Clipboard
shortcut-input-selection-or-clipboard-description = Prefer selected text, fallback to clipboard
send-content-screenshot = Screenshot
shortcut-input-screenshot-description = Select a screen area, OCR it, then send
