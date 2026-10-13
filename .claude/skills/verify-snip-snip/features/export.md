# Export

Copy writes the annotated/cropped image to the clipboard; Copy & Close closes
only after success. Save uses a native picker and writes a PNG at source
resolution. Failures keep the document available for retry.

## Sub-features

- Copy supplies PNG/bitmap clipboard formats; Copy & Close preserves the payload.
- Save starts in Pictures' snip-snip folder (home fallback), with a local timestamp
  name and numeric suffix for collisions.
- Cancel writes nothing; clipboard/folder/picker/write errors leave the editor open.
- Export finishes text and cancels unfinished drags; view controls are excluded.

## How to get to it (user POV)

`mod+c`, `mod+shift+c`, `mod+s`, or Copy / Copy & Close / Save toolbar controls.

## Driving it with snipctl

Preconditions: healthy owned session, fixture snip with an annotation or crop.

- **Copy/Save.** `key mod+c`, `clipboard export-copy`, `save export-save`, then
  `compare export-copy export-save`. Inspect the saved dimensions/content and
  capture the final editor state. One comparison per flow is enough.
- **Copy & Close, when affected.** Activate it, use `doctor` to confirm the
  editor closed while the app remains healthy, then read the clipboard and
  `compare export-save export-copy-close`. Start another snip.
- **Cancel, when affected.** `cancel-save` dismisses the native picker and asserts
  the editor remains open. Confirm editing still works and no export was created.
- **Defaults, when affected.** Inspect the picker's initial folder/name before
  replacing it. `save` writes an evidence path and cannot prove the suggested defaults.
- **Errors, when affected.** Controlled OS-adapter/editor tests cover failures.
  Exercise native failures where practical; report relevant untested cases.

## Gotchas

- `save` refuses existing evidence names and waits for a decodable image.
- Status text may truncate paths; inspect the file or clipboard itself.
- `key esc` focuses the editor. Use `cancel-save` to dismiss its modal picker.
- A globally reserved completion shortcut may not reach the editor. Use
  `probe-shortcut ctrl+shift+c` on Windows to diagnose it, then check the visible
  toolbar control. Report shortcut delivery separately when it is in scope.

For clipboard format/lifetime changes, add a native application-paste check on
Windows: `paint-launch EXE`, `paint-key ctrl+v`, `paint-shot paste-view`.
Use fresh screen coordinates with `paint-click` to crop excess blank canvas in
Paint, then `paint-key ctrl+s`, `paint-key ctrl+a`, `paint-type FULL_PATH`,
`paint-key enter` to save under the run directory. `compare` its decoded PNG
with the clipboard PNG. Repeat after Copy & Close only when payload lifetime
changed. For a new Paint document, send `paint-key esc ctrl+n`, wait for the new
window, then paste in a separate call. Cleanup stops only the Paint process
this run started. The helper recognizes English `Save as` dialogs.
