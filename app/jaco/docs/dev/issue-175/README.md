# Jaco shortcut settings and temporary conversations

[English](README.md) | [简体中文](README.zh-CN.md)

Issue [#175](https://github.com/suxiaoshao/gpui/issues/175) unifies Jaco's chat input, shortcut editor, and temporary-conversation experience around one presentation model while keeping application state ownership explicit.

Status: the final design, implementation, automated verification, removed-API audit, and documentation-consistency audit are complete. Hands-on UI validation was intentionally not performed for this pass.

The final design has three main parts:

- `ChatForm` is a presentation-only shell composed with `ControlSlot`.
- Generated form stores own current typed values; owning bound controls keep the UI synchronized.
- Shortcut-created conversations use the same popup temporary-window runtime as ordinary temporary conversations.

See the [complete design](design.md) for concepts, ownership, and final behavior.

## Documents

### Final design

- [Design (English)](design.md)
- [设计（简体中文）](design.zh-CN.md)

### Issue implementation plans

These plans cover issue #175 product work only. The application-wide form-library migration is documented separately in [`../gpui-form-migration.md`](../gpui-form-migration.md).

- [chat-form-refactor.md](chat-form-refactor.md)
- [run-settings.md](run-settings.md)
- [temporary-window-runtime.md](temporary-window-runtime.md)

The implementation documents are not part of the stable architecture contract.
