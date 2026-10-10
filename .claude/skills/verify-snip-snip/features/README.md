# snip-snip verification map

The maintained source for verifying snip-snip's user-facing behavior. Read this index, then use the matching feature file as the recipe. `snipctl` means `cargo run -q --manifest-path .claude/skills/verify-snip-snip/snipctl/Cargo.toml --`, run from the repo root.

## Baseline preconditions

- Release binary built from the current source; `snipctl launch` succeeded and `snipctl doctor` ends with `doctor: OK`.
- No editor window is open (`doctor` lists windows).
- Unless a recipe says otherwise, start each snip with `snipctl target` then `snipctl snip 200 200 600 300`: a 600x300 image whose left 300 columns are blue and right 300 are green.

## Driving conventions

- Every action goes through `snipctl`; it logs each step to `<run>/steps.log`.
- Image coordinates for `drag`/`click` are pixels of the snipped image, origin top-left.
- Expected `inspect` counts in the recipes were measured on Windows at 100% scaling, where one screen unit is one image pixel. `snip` prints the image size it expects; on a 2x display, sizes double and counts quadruple.
- Prefer keyboard shortcuts (`p a r t c`, `mod+…`) to toolbar clicks: the toolbar has no accessibility handles, so keys are the stable handle.
- Run `snipctl cleanup` after the session, and after any failed step before retrying.

## Proof and skip reporting

- Pair each action with a `shot` of the resulting editor state.
- Exported pixels are the ground truth: `clipboard` or `save`, then `inspect`, with expected counts stated first.
- Name artifacts by feature ID (e.g. `annotate-rect`).
- Report an entry point snipctl cannot reach (toolbar buttons, right-click) as skipped with the reason, never as verified through another path.

## Feature entry contract

Each feature file has an H1 title, one paragraph of user-visible behavior, then exactly four H2s in order: `Sub-features`, `How to get to it (user POV)`, `Driving it with snipctl` (starting with `Preconditions:`), `Gotchas`.

## Features

- [Snip](./snip.md): hotkey, frozen overlay, region selection, cancel.
- [Annotate](./annotate.md): pencil, arrow, rectangle, text.
- [Crop](./crop.md): crop to a sub-rectangle, applied to exports.
- [Undo and redo](./undo-redo.md): history across annotations and crop.
- [Export](./export.md): copy to clipboard and save as PNG.
