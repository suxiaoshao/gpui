# State and Interaction

Use component selection, disabled and loading builders, existing focus behavior, and the window/root overlay components. Title-bar behavior varies by platform; app-owned `start_window_move` requires `WindowOptions::app_owns_titlebar_drag = true`. Interactive children must handle propagation intentionally.

Prefer the component's delegate interfaces for lists and selectors. Never synchronously update `ListState` from its own delegate callback; defer owner mutations that can re-enter it.

A component entity can contain three distinct channels:

- Form/domain owns the business value.
- App/catalog owns options and capabilities.
- Native component owns focus, query, scroll, IME and popup state.

Submit and validate the domain value. Replacing options must not become user input or choose a business fallback. When replacing a Combobox delegate leaves its cached selection stale, project the authoritative value with `ComboboxState::set_selected_values` after replacing the options.

Use gpui-form bindings for their built-in source suppression and deferred writes. Custom integrations must prevent synchronous feedback loops without duplicating the Form adapter's routing.
