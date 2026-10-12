---
name: verify-snip-snip
description: Drive the real snip-snip desktop app (global hotkey, capture overlay, editor, copy/save) with real mouse and keyboard input and capture proof, on Windows or macOS. Use to confirm a change works end to end, reproduce a bug as a user would, or screenshot the app.
---

# Verify snip-snip

snip-snip is a background GPUI app with a tray/menu-bar icon and a configurable capture shortcut, initially `Alt+Shift+S`. A user presses the hotkey, drags a region on the frozen-screen overlay, annotates in the editor, then copies or saves. Every step here drives that same path with injected OS input: the real hotkey, the real overlay, the real save dialog. For tray controls and shortcut configuration, read [App shell](features/app-shell.md).

All driving goes through `snipctl`, a Rust CLI in `snipctl/` next to this file (its own Cargo project, outside the product workspace). From the repo root:

```sh
cargo run -q --manifest-path .claude/skills/verify-snip-snip/snipctl/Cargo.toml -- help
```

Below, `snipctl X` is shorthand for that invocation. Cargo rebuilds it only when its source changes.

## Ground rules

- **One run at a time.** The hotkey is machine-wide and input is the real mouse and keyboard, so two runs (or a user's own snip-snip) cannot coexist. `launch` refuses when any snip-snip is already running; ask the user before stopping one you did not start.
- **The desktop is shared.** Runs move the user's cursor and type into the foreground window. Tell the user before a run so they keep hands off.
- **Coordinates.** `snip` takes screen units (physical pixels on Windows, points on macOS) on the primary monitor. `drag`, `click`, and `double-click` map image pixels in Fit; use `key mod+0` before them. After zoom/pan, use `drag-screen`, `click-screen`, and `scroll-screen` with absolute coordinates derived from a current screenshot and `layout`.
- **Platforms.** Windows is proven end to end. The macOS code compiles but has never run: the first Mac run must prove it, fix what breaks, and remove the warning in `snipctl/src/platform/macos.rs`. macOS needs Accessibility and Screen Recording permission for the terminal running snipctl. `mod` in chords is Ctrl on Windows and Cmd on macOS.

## Launch

```sh
cargo build --release -p snip-gpui   # once per app source change
snipctl launch                       # new run dir target/verify/<run>/, waits for "snip-snip running"
```

`launch` starts `target/release/snip-snip` with its output in `<run>/app.log` and records its pid and start time; later commands act on that run (pointer: `target/verify/current`).

## Doctor

Run `snipctl doctor` first, and again whenever anything looks off. It is read-only and ends with `doctor: OK` only when: our pid is alive and is this repo's binary, the ready line is logged with no errors, no other snip-snip runs, the desktop is unlocked (Windows only), and the binary is newer than every file under `crates/` (otherwise: rebuild, relaunch).

## Drive

The canonical loop:

```sh
snipctl target                    # fixture at screen 100,100, 800x500: left half #2E86DE, right half #20BF6B
snipctl snip 200 200 600 300      # hotkey, drag on overlay, wait for editor; closes the fixture
snipctl key r                     # tool keys p a r t c; chords like mod+z mod+shift+z mod+c
snipctl drag 100 80 500 220       # in image pixels
snipctl shot 01-rectangle         # editor window -> <run>/01-rectangle.png
snipctl save 02-saved             # mod+s, enters <run>/02-saved.png in the native dialog, waits for the file
snipctl inspect 02-saved          # size + exact-color counts: blue, green, ink (#E5484D)
snipctl close
```

The fixture gives every snip known pixels, so `inspect` turns "it looks right" into numbers: on Windows at 100% the snip above yields `600x300 blue=90000 green=90000`, and annotations add `ink`. Snip without `target` to capture whatever is on screen.

Per-feature recipes, entry points, and gotchas live in [`features/README.md`](features/README.md). Read the matching feature file before driving it.

`drag X1 Y1 X2 Y2 shift` holds Shift through the gesture. `drag-screen` also
accepts a held key, including `space` for pan. `drag-preview` captures the
live drawing before mouse release. `hover-screen` settles the pointer before
Windows `cursor` reads its native shape; `resize-editor` takes client-area
dimensions. The Paint paste recipe lives in [Export](features/export.md).

For the Windows shell, use `snipctl verify-shell` with no app running; it owns
launch, restart, checks, evidence, and cleanup. Follow [App shell](features/app-shell.md)
for its completion criteria and remaining visual review.

## Evidence

Everything lands in `target/verify/<run>/`: `steps.log` (each action, timestamped), `app.log` (the app's stderr: hotkey, capture, render, export timings), screenshots, and exported PNGs.

A proof:

- Drives the user path (hotkey, overlay, keys, dialog), never internal state.
- Captures the action and the result: a `shot` after each meaningful step, not only the end.
- Verifies the side effect outside the app: `clipboard` then `inspect` for Copy, `save` then `inspect` for Save. The editor's status text alone is not proof.
- States the expected `inspect` numbers before reading them.

`cargo clean` deletes `target/verify`; copy evidence elsewhere if it must outlive the build tree.

## Cleanup

```sh
snipctl close      # if an editor is open
snipctl cleanup    # stops only the app and fixture pids this run recorded
```

Cleanup never touches evidence; it prints the run dir it kept. After a failed step, run `cleanup` before retrying so no stray instance keeps the hotkey.
It also stops Paint when `paint-launch` started it for this run.

## Changing snipctl

Shared code lives in `snipctl/src/`; everything OS-specific (focus, the editor's content area, coordinate units, the save dialog, doctor checks) is in `snipctl/src/platform/`. `screen.rs` mirrors the editor's layout constants from `crates/snip-gpui/src/editor.rs`; when those change, update it in the same commit. Check both platforms compile: `cargo check --target aarch64-apple-darwin` in `snipctl/`.
