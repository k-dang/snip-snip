# Export

`Ctrl/Cmd+C` copies the annotated, cropped image to the clipboard as PNG and bitmap formats. `Ctrl/Cmd+Shift+C` copies and closes only after success. `Ctrl/Cmd+S` opens the native save dialog in the screenshot folder with a readable local timestamp and writes a PNG at the original screenshot resolution. Export failures keep the editor open with an error.

## Sub-features

- `export-copy` the clipboard holds PNG plus bitmap formats; status reads `Copied to clipboard`.
- `export-copy-close` a successful clipboard write closes only the current editor; the copied image remains available, and another snip can be started.
- `export-copy-failure` failed Copy or Copy & Close keeps the editor open and reports the error.
- `export-save` the dialog starts in Pictures' `snip-snip` subfolder (home fallback when Pictures cannot be resolved) and suggests `snip-YYYY-MM-DD_HH-mm-ss-SSS.png`, adding a numeric suffix for existing names; the chosen file is written; status reads `Saved <path>`.
- `export-save-cancel` cancelling the dialog writes nothing.
- `export-save-failure` folder preparation, picker, and write errors are visible and leave the document available for retry.

## How to get to it (user POV)

- `Ctrl/Cmd+C`, `Ctrl/Cmd+Shift+C`, or `Ctrl/Cmd+S` in the editor, or the `Copy`, `Copy & Close`, and `Save…` toolbar buttons.

## Driving it with snipctl

Preconditions:

- Baseline snip open with one rectangle drawn (`snipctl key r`, `snipctl drag 100 80 500 220`).

- **Copy.** `snipctl key mod+c`, `snipctl clipboard export-copy`. On Windows the step log lists `PNG, CF_DIBV5, CF_BITMAP, CF_DIB`; macOS lists none. `snipctl inspect export-copy` gives `600x300 blue=87840 green=87840 ink=2160`.
- **Save.** `snipctl save export-save`. It presses `mod+s`, enters `<run>/export-save.png` in the native dialog, and waits for the file. `snipctl inspect export-save` matches the copy exactly.
- **Proof.** `snipctl shot export-after-save`: the status area starts with `Saved`.
- **Copy & Close.** `snipctl key mod+shift+c`, then `snipctl doctor`: no editor remains and the app is still healthy. `snipctl clipboard export-copy-close` and `snipctl inspect export-copy-close` must match the saved image. Start another fixture snip to prove the hotkey remains available.
- **Cancel.** `snipctl cancel-save` opens the native picker, dismisses it, and asserts the editor stays open. `snipctl shot export-save-cancel` records the resulting editor; no PNG should have been created by cancellation.
- **Windows application paste.** Locate Paint's installed executable, then `paint-launch EXE` starts an owned process. `paint-key ctrl+v` pastes; `paint-shot paste-view` records it. Click Paint's Crop button using `paint-click X Y` with fresh screenshot coordinates to remove excess blank canvas. `paint-key ctrl+s` opens its save dialog; `paint-key ctrl+a`, `paint-type FULL_PATH`, and `paint-key enter` save a PNG under the run directory. Compare its decoded pixels to the source clipboard PNG. Repeat after Copy & Close; before a new Paint document use `paint-key esc ctrl+n`, then send `ctrl+v` in a separate call after the new window is ready. Cleanup stops only the Paint process this run owns.

## Gotchas

- `save` refuses an existing file name so an overwrite prompt never stalls the dialog. Use a new name per save.
- At the default window width the status text is truncated (`Saved C…`); the file and `inspect` are the proof.
- Default folder/name inspection needs native picker inspection; `save` replaces the suggested name with an evidence path and cannot prove the defaults by itself.
- Use `cancel-save` for cancellation. Generic `key esc` focuses the editor and can close it instead of dismissing a Windows save dialog.
- Clipboard readback proves payload persistence after closure, but application-paste compatibility needs a separate native paste check.
- OS export failures have deterministic rendered-editor coverage; snipctl does not inject folder, picker, or clipboard errors. Report native failure cases as skipped unless they were actually exercised.
- A globally reserved Ctrl+Shift+C chord may never reach the editor. Record that shortcut check as blocked; use screenshot-derived toolbar coordinates with `snipctl click` to verify the native completion action separately. In the 600x300 fixture run at 65% fit, `click -90 -57` reached Copy & Close; derive fresh coordinates when layout changes.
- Windows `probe-shortcut ctrl+shift+c` checks the global reservation. A successful probe immediately unregisters; `AlreadyRegistered` confirms the chord is unavailable.
- Paint's crop shortcut did not perform the action in the native run; click its visible Crop control. The Paint helper targets an English `Save as` dialog when present.
