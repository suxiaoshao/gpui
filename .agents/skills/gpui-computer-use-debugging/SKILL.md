---
name: gpui-computer-use-debugging
description: Inspect or debug GPUI desktop behavior using the current local build and isolated test data when runtime UI validation is requested or required.
---

# GPUI Computer Use Debugging

Compilation alone does not establish visual or interaction correctness.

## Artifact and scenario

- Use the current local `.app` produced by `xtask bundle <app>`, normally under `target/release/bundle/macos/`. Reuse it only while the relevant source state matches; an installed `/Applications` copy is not evidence for the local change.
- App-local test cases supply fixtures, field values, navigation, expected results and cleanup. Feiwen indexes them at `app/feiwen/docs/tests/README.md` and feature context at `app/feiwen/docs/features/README.md`.
- Seed prerequisite records when creating them is outside the scenario.

## Isolated data

- Use a temporary config/data directory, test database and local mocks through the app's supported overrides. Do not repurpose `HOME`, `CARGO_HOME` or `RUSTUP_HOME` as isolation controls.
- Establish isolation before interacting with data. Real app data, credentials, Cookies and production crawl targets require explicit authorization.
- A requested scenario can include deleting or regenerating its disposable fixtures. Fixture use does not authorize real network crawls, paid API calls, external messages or persistent OS shortcut registration.

## Build identity and evidence

- When using `cua_repl`, `cua.getApp("<absolute local .app path>")` selects the artifact. Confirm attachment identity from artifact/process and launch evidence: a bundle ID alone cannot distinguish installed and local copies, and `cargo run` logs do not establish the identity of a UI attachment.
- Accessibility state supports control identification; screenshots support layout, clipping and rendering evidence. Obtain both only when the scenario needs both.
- Record the artifact, identity evidence, isolation method, tested interactions, observed results and limitations. Separate observations from inference when attachment or accessibility limits leave a check unresolved.
- For an authorized fix, reproduce the affected scenario on the rebuilt artifact. Clean up this run's disposable data and mocks while retaining requested evidence.
