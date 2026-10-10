# Undo and redo

`Ctrl+Z` steps back one completed edit (an annotation, a finished text label, or a crop); `Ctrl+Shift+Z` or `Ctrl+Y` steps forward. A new edit after an undo discards the redo branch.

## Sub-features

- `history-undo` `Ctrl+Z` removes the last completed edit from the preview and from exports.
- `history-redo` `Ctrl+Shift+Z` and `Ctrl+Y` restore it.
- `history-branch` an edit after undo clears redo.
- `history-text` a text label is one edit, committed when finished.

## How to get to it (user POV)

- `Ctrl/Cmd+Z`, `Ctrl/Cmd+Shift+Z`, and `Ctrl+Y` (Windows only) in the editor. There are no toolbar buttons.

## Driving it with snipctl

Preconditions:

- Baseline snip open with one rectangle drawn (`snipctl key r`, `snipctl drag 100 80 500 220`).

- **Undo.** `snipctl key mod+z`, `snipctl shot history-undo`. The rectangle is gone.
- **Redo.** `snipctl key mod+shift+z`, `snipctl shot history-redo`. It is back.
- **Ctrl+Y (Windows).** `snipctl key mod+z ctrl+y`, `snipctl shot history-ctrl-y`. Back again.
- **Branch.** `snipctl key mod+z`, then `snipctl key a`, `snipctl drag 50 250 250 50`, then `snipctl key mod+shift+z`, `snipctl shot history-branch`. Only the arrow shows.
- **Proof in pixels.** Copy and `inspect` after each state: `ink=0` with nothing drawn, `ink` above zero when restored.

## Gotchas

- A screenshot alone cannot tell "undone" from "never drawn"; take one before and after each history step.
- Undo while editing text first finishes the label (committing it), then undoes.
