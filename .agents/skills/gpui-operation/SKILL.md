---
name: gpui-operation
description: Implement or modify gpui-operation and its deliberate application integrations.
---

# GPUI Operation

Use the [README](../../../crates/gpui-operation/README.md) and [guide](../../../crates/gpui-operation/docs/guide.md) for current families and legal messages. Ordinary async code need not adopt this crate.

- `refresh` repeats a read; `repair` accepts caller-selected recovery. The app chooses recovery and UI behavior.
- Keep one complete runtime enum in its owner. Its running variant owns the lifecycle-critical task; the application constructs it and routes completion. Do not mirror phase/data/task state.
- Construct and install a legal start without yielding or synchronous owner re-entry. Route completion through a weak owner or another route that avoids a strong reference cycle.
- Under abort-on-drop task ownership, cancellation removes the completion route. A producer that survives cancellation needs its own freshness guard. Cancellation cannot undo external side effects.
- Complete runtime transitions install the final state before dropping user-owned payloads. Preserve this ordering when changing the crate.

For UI integration, distinguish empty successful data from unavailable data and decide whether retained data remains usable after failure. Cover affected transitions and task lifetime; keep changed public English/Chinese docs aligned.
