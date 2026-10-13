# snip-snip verification map

The maintained source for verifying snip-snip's user-facing behavior. Read this index, then use the matching feature file as the recipe. `snipctl` means `cargo run -q --manifest-path .claude/skills/verify-snip-snip/snipctl/Cargo.toml --`, run from the repo root.

## Baseline preconditions

- Release binary built from the current source; `snipctl launch` succeeded and `snipctl doctor` ends with `doctor: OK`.
- No editor window is open (`doctor` lists windows).
- Unless a recipe says otherwise, start each snip with `snipctl target` then `snipctl snip 200 200 600 300`. On Windows at 100% scale, this yields a 600x300 image whose left 300 columns are blue and right 300 are green.

## Driving conventions

- Every action goes through `snipctl`; it logs each step to `<run>/steps.log`.
- Image coordinates for `drag`/`click`/`double-click` are pixels of the snipped image in Fit, origin top-left. Reset with `key mod+0`; after navigation use absolute `*-screen` commands instead.
- A `click` outside those bounds can target the toolbar. Derive its coordinates from a current screenshot and `layout`, and record the action and resulting behavior; toolbar coordinates vary with wrapping.
- Expected `inspect` counts in the recipes were measured on Windows at 100% scaling, where one screen unit is one image pixel. `snip` prints the expected image size. On macOS at 2x scale, the baseline selection is 1199x599 because each drag end includes one screenshot pixel. Compare colors against a native fixture screenshot; display profiles can change the RGB values.
- Prefer keyboard shortcuts (`p a r t c`, `mod+…`) to toolbar clicks: the toolbar has no accessibility handles, so keys are the stable handle.
- Run `snipctl cleanup` after the session, and after any failed step before retrying.

## Proof and skip reporting

- Pair each action with a `shot` of the resulting editor state.
- Exported pixels are the ground truth: `clipboard` or `save`, then `inspect`, with expected counts stated first.
- Name artifacts by feature ID (e.g. `annotate-rect`).
- Report an entry point snipctl cannot reach as skipped with the reason, never as verified through another path. A toolbar click does not prove that its keyboard shortcut was delivered.

## Verification status — 2026-10-12

All MVP features are implemented. The release build, all 88 automated tests,
formatting, and Clippy pass; native macOS verification remains partial.

- macOS passed: physical shortcut delivery, Settings recording/application,
  restart persistence, Canadian/Dvorak input-source switching, capture, and
  clipboard readback matching every pixel of the native fixture region.
- macOS remaining: annotation selection/movement/handles, styles, text/IME,
  zoom/pan and resizing, crop editing, Save, Copy & Close, menu-based capture
  and Quit, and shortcut conflict/error recovery. Native pinch needs a touchpad.
- Windows remaining: native pinch and delivery of the Copy & Close shortcut
  (`Ctrl+Shift+C` was globally reserved on the verification machine). Its
  toolbar completion flow was verified.

macOS evidence is in `target/verify/1791843397808/` (capture and Settings) and
`target/verify/1791843849110/` (restart, source switching, and exact pixel
comparison). The latter contains the Swift verification scripts and
`pixel-comparison.txt`. Cleanup restored the original shortcut preferences,
restored Canadian input, disabled temporary Dvorak, and stopped the owned app.

## Feature entry contract

Each feature file has an H1 title, one paragraph of user-visible behavior, then exactly four H2s in order: `Sub-features`, `How to get to it (user POV)`, `Driving it with snipctl` (starting with `Preconditions:`), `Gotchas`.

## Features

- [Snip](./snip.md): hotkey, frozen overlay, region selection, cancel.
- [App shell](./app-shell.md): tray/menu-bar controls, Settings, capture shortcut, Quit.
- [Annotate](./annotate.md): pencil, arrow, rectangle, text.
- [Navigation](./navigation.md): wheel zoom, Space-pan, Fit/100%, resizing, and zoomed editing.
- [Crop](./crop.md): crop to a sub-rectangle, applied to exports.
- [Undo and redo](./undo-redo.md): history across annotations and crop.
- [Export](./export.md): copy, copy and close, save defaults, PNG export, and failure handling.
