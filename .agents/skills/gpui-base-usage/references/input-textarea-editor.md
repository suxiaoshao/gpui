# Input, Textarea, and Editor

The target splits text controls by responsibility:

| Job | State/control | Boundary |
| --- | --- | --- |
| Single-line value, masking, validation, number stepping | `gpui_base::input::{InputState, Input}` | Enter can submit; no ordinary multi-line content |
| Ordinary multi-line text, rows, wrapping, auto-grow | `gpui_base::input::{TextareaState, Textarea}` | No source-code gutter, folding, or language configuration |
| Source code, language, gutter, folding, decorations, diagnostics/LSP seams | `gpui_base::input::{EditorState, Editor}` | Not a generic textarea |

Create the state Entity once, retain it in the owner, and render the matching
control. The state owns editing behavior, focus, selection, keyboard input,
IME, and incomplete native text. The form/domain owns the typed business value;
the app owns persistence and errors.

Base input controls require application-supplied `InputEditorStyle` and frame
composition. Styled prefix/suffix slots, mask toggles, clear buttons, themed
borders, and ready-made sizing belong to full `gpui-component` and must not be
assumed here.

Subscribe to `InputEvent` only for product consequences. Do not maintain a
second component-value authority; project programmatic values through the
state API and avoid synchronously re-entering the same Entity from its event
callback.

For a Form integration, keep a non-clone binding as the projection lifetime
owner and preserve focus/IME/selection in the native state. Use the `gpui-form`
skill for that adapter contract.
