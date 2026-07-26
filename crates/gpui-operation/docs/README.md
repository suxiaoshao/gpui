# gpui-operation documentation

## Library documentation

- [User guide (English)](guide.md)
- [使用指南（中文）](guide.zh-CN.md)

The guides describe the current public contract:

- message-driven transitions for complete runtime enums and named states;
- separate refresh-only and repair-capable operation families;
- one complete, predefined runtime `Operation` enum per family;
- `Fetching<Previous, Task>` and
  `Repairing<Previous, Repair, Task>`;
- clone-free completion and cancellation;
- in-place `Transition<Message>` delivery to complete runtime enums;
- ignored runtime messages that preserve state and drop their payloads, with
  optional tracing;
- exact-Ready domain messages without mutable Data access;
- separate Entity, ordinary Global, and `gpui-store::Store<S>` usage
  for both operation families.

The current design does not include `OperationSource`, library-owned task drivers,
opaque completions, command-style runtime methods, attempt identity, or one
universal cross-family `Operation<S>`.
