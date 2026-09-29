---
name: gpui-computer-use-debugging
description: Reproduce or verify GPUI desktop behavior when runtime evidence is needed for the requested task.
---

# GPUI Computer Use Debugging

Choose a current local build and confirm the attached process belongs to it. Use `cargo run` or an existing matching build for ordinary UI work; use `xtask bundle <app>` when testing packaging, bundled resources or OS launch behavior. With `cua_repl`, select the intended app path; bundle ID alone cannot distinguish installed and local copies.

Use app-supported config/data overrides and disposable fixtures. Real user data, credentials and live external operations need authorization. Feiwen scenarios are indexed at `app/feiwen/docs/tests/README.md`; read only the affected scenario.

Reproduce the reported behavior or check the primary path needed for delivery. Use accessibility state for control behavior and screenshots for visual claims. Tool capture failures establish a limitation, not an application defect.

After a fix, repeat the affected scenario and stop when it passes. Clean up this run's disposable fixtures and processes. Report the observed result and any limitation material to the task; no full UI or packaging sweep is implied.
