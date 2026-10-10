# Export

`Ctrl+C` copies the annotated, cropped image to the clipboard as PNG and DIB; `Ctrl+S` opens the native save dialog and writes a PNG at the original screenshot resolution. The toolbar status reports the result.

## Sub-features

- `export-copy` the clipboard holds PNG plus bitmap formats; status reads `Copied to clipboard`.
- `export-save` the dialog suggests `snip-<unix time>.png` in the home folder; the chosen file is written; status reads `Saved <path>`.
- `export-save-cancel` cancelling the dialog writes nothing.

## How to get to it (user POV)

- `Ctrl/Cmd+C` / `Ctrl/Cmd+S` in the editor, or the `Copy` / `Save…` toolbar buttons.

## Driving it with snipctl

Preconditions:

- Baseline snip open with one rectangle drawn (`snipctl key r`, `snipctl drag 100 80 500 220`).

- **Copy.** `snipctl key mod+c`, `snipctl clipboard export-copy`. On Windows the step log lists `PNG, CF_DIBV5, CF_BITMAP, CF_DIB`; macOS lists none. `snipctl inspect export-copy` gives `600x300 blue=87840 green=87840 ink=2160`.
- **Save.** `snipctl save export-save`. It presses `mod+s`, enters `<run>/export-save.png` in the native dialog, and waits for the file. `snipctl inspect export-save` matches the copy exactly.
- **Proof.** `snipctl shot export-after-save`: the status area starts with `Saved`.

## Gotchas

- `save` refuses an existing file name so an overwrite prompt never stalls the dialog. Use a new name per save.
- At the default window width the status text is truncated (`Saved C…`); the file and `inspect` are the proof.
- `export-save-cancel` has no snipctl command; report it as skipped.
