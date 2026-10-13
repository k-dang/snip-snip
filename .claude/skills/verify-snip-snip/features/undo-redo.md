# Undo and redo

Undo reverses one completed edit; redo restores it. An edit after undo discards
the redo branch. View changes preserve document history.

## Sub-features

- Undo/redo for annotation, text, style, and crop changes.
- One history step per completed gesture or text session.
- New edits clear the redo branch; Windows also supports Ctrl+Y.

## How to get to it (user POV)

`mod+z`, `mod+shift+z`, or Ctrl+Y on Windows.

## Driving it with snipctl

Preconditions: fixture snip with one rectangle (`key r`, `drag 100 80 500 220`).

- **Undo/redo.** `key mod+z` removes the rectangle; `key mod+shift+z` restores it.
  Observe both transitions and capture the restored result.
- **Branch, when history changes.** Undo, draw an arrow, then redo: only the arrow
  remains. Check Ctrl+Y when Windows shortcut handling changes.
- **Text, when affected.** A completed text session undoes in one step.
- **Exact restoration, when investigating history correctness.** Copy a reference,
  edit, undo, then copy the restored state and `compare reference restored`.
  One comparison proves restoration; exporting every intermediate state adds little.

## Gotchas

- Observe the original edit before undoing, so disappearance proves a transition.
- Undo while typing first finishes the label, then undoes it.
