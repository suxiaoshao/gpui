# Jaco chat input and shortcut architecture

[English](design.md) | [简体中文](design.zh-CN.md)

## 1. Goals

This design gives ordinary chat, new conversations, temporary conversations, and shortcut editing one consistent input experience without forcing them to share business state they do not need.

It also ensures that a shortcut-created conversation behaves as a temporary conversation at runtime, rather than appearing in a normal application window.

## 2. Presentation architecture

### `ChatForm`

`ChatForm` is a presentation-only component. It owns layout, styling, and local visual composition, but it does not own:

- a `gpui-form` session or application input model;
- provider/model/project catalogs;
- persistence or database access;
- shortcut, conversation, or agent-run business logic.

The caller passes component state and event handlers for the controls it wants to present. This keeps the visual experience consistent while allowing each surface to own the correct data.

### `ControlSlot`

Every optional control is represented by one of three states:

- `Hidden`: absent from layout and interaction;
- `Disabled`: rendered consistently but rejects user interaction;
- `Enabled`: rendered and interactive.

This single abstraction controls visibility, disabled behavior, focus eligibility, and layout. Surfaces do not maintain separate boolean combinations for the same control.

## 3. Surface composition

| Surface | Project | Composer/attachments | Run settings | Primary action |
| --- | --- | --- | --- | --- |
| New conversation | Enabled | Enabled | Enabled | Enabled |
| Existing conversation | Hidden | Enabled | Enabled | Enabled |
| Temporary new conversation | Hidden | Enabled | Enabled | Enabled |
| Shortcut editor | Hidden | Disabled | Enabled | Disabled |

The shortcut editor uses the same visual shell and run-setting controls as chat, but it edits only configuration that belongs to a shortcut. Disabled controls may display context but cannot mutate it through keyboard, paste, IME, or action handlers.

## 4. State ownership

| State | Owner |
| --- | --- |
| Composer text | generated parent form store |
| Composer interaction | composer bound control |
| Attachments | generated parent form store |
| Attachment workflow | chat-input controller |
| Selected model, reasoning effort, token budget, approval/tool access | generated parent form store |
| Run-setting interaction and options | owning bound controls and run-settings controller |
| Project selection | new-conversation form store and bound control |
| Options/catalogs/capabilities | application `gpui-store` stores/controllers |
| Current typed value, baseline, validation and submit runtime | one generated parent form store per editing surface |
| Focus and error visibility | visible page/dialog and concrete component instances |
| Conversation, agent run, persistence | application services/stores |

There is no parallel component-owned business value or string draft. Owning bound controls synchronize their typed value with the generated store and keep only interaction/configuration state locally.

## 5. Run settings

The generated parent form owns the typed values used by model execution:

- model selection;
- reasoning effort;
- exact integer token budget;
- approval/tool-access mode.

`RunSettingsController` owns the bound controls and binds them to the parent form's nested fields. It does not create a second nested form store.

Model options and capabilities remain configuration. Refreshing them does not silently select another model or rewrite the current selection. If the selected model is unavailable or incompatible, validation reports an explicit error.

## 6. Validation, focus, and submit

On a component event, the bound control writes the typed value to its `FormField`; generated validation then reads the updated parent model.

On submit, `prepare_submit` clones the parent form value once and uses that same value for:

- validation;
- model/capability/attachment compatibility checks;
- transformation into the application command;
- persistence or run creation.

The submit path does not reread the database or catalog to choose a fallback model. A missing or disabled selection is an error.

The form store returns error paths but never focuses UI. The active page/dialog maps a path to its currently visible bound control. This remains correct when the same data is presented by multiple component instances.

Field rendering reads static `required` metadata from the generated schema and data errors/pending state from the form. Jaco translates error message keys and bound controls combine them with local interaction state. Field-scoped async validation can drive an input spinner through `is_validating_at(path)` without moving spinner or focus state into the form.

## 7. Catalog and persistence boundaries

Provider, model, project, and capability catalogs are committed application state owned by `gpui-store` stores/controllers. Pages consume typed snapshots from those stores.

Ordinary UI paths do not query the database directly for catalog data. Form validation may receive a captured catalog snapshot as context, but it cannot mutate that catalog or replace a selected value.

Successful persistence flows through application services/stores. Only after success does the editing surface rebase its generated form store with the saved typed value.

## 8. Temporary-window runtime

A global temporary-conversation hotkey and a configured shortcut that creates a conversation both route into the same popup `TemporaryWindow` runtime.

The runtime owns popup visibility, the temporary conversation list, current route, and focus restoration. Hotkey code requests an action; it does not downcast window internals or fall back to a normal main window.

Conversation-list keyboard and pointer interactions both request selection through the owning temporary controller. Delegate callbacks do not synchronously update their own `ListState`; route/focus changes are applied outside the active delegate update.

## 9. GPUI lifecycle invariants

- A callback never synchronously updates the entity currently being updated.
- Picker/list delegates emit intent; their owner applies cross-entity state changes.
- Deferred work captures weak entities and rechecks liveness.
- Opening/closing popup UI does not steal focus permanently from the intended search/composer field.
- Page-level subscriptions are retained by the page/controller; component-internal subscriptions are retained by the component.

These rules are part of the architecture because violating them causes `already being updated` or `RefCell already borrowed` failures.

## 10. Non-goals

This design does not redesign the prompt system, provider adapters, agent execution protocol, MCP per-tool permissions, database schema, or visual theme. It also does not add another generic binding framework between Jaco and `gpui-form`.

## 11. Final invariants

- All chat-like surfaces share one visual language through `ChatForm` and `ControlSlot`.
- Each surface owns only the values it can edit.
- Current values, configuration, validation runtime, and persistence have separate owners.
- One editing surface has one parent form session.
- Catalog refresh never silently changes user selection.
- The value that passes validation is the value submitted.
- Shortcut-created conversations always use the popup temporary-window lifecycle.
- GPUI event handlers do not perform nested updates on the active entity.
