---
name: verify-snip-snip
description: Drive the real snip-snip desktop app with OS mouse/keyboard input to verify a change, reproduce a bug, or capture screenshots on Windows or macOS.
---

# Verify snip-snip

Use `snipctl`, the Rust CLI beside this file. From the repo root:

```sh
cargo run -q --manifest-path .claude/skills/verify-snip-snip/snipctl/Cargo.toml -- help
```

Below, `snipctl X` means that invocation. Read the matching recipe in
[the feature map](features/README.md) when verifying a particular feature.

## Shared desktop

- Run one session at a time. `launch` refuses if another snip-snip owns the
  global shortcut. Ask before stopping an instance you did not start.
- Announce native runs: they move the user's cursor and type into the foreground
  window. Keep desktop driving sequential.
- `cleanup` stops only processes this run owns and retains the evidence.
  Use it after success or failure.

## Build and launch

```sh
cargo build --release -p snip-gpui
snipctl launch
snipctl doctor
```

`launch` starts this checkout's release binary and creates `target/verify/<run>/`.
`doctor` checks ownership, readiness, freshness, competing processes, and desktop
availability. Run it again if input stops working or the app behaves unexpectedly.
On macOS, capture/input needs Screen Recording and Accessibility permissions for
snipctl's host. Native macOS save-panel automation remains unverified.

## Routine check

Run this short flow for broad editor changes; for a focused change, exercise the
changed behavior and its relevant output instead. Observe crop, undo, and redo
as they happen, then capture the final editor and export once through each path.

```sh
snipctl target
snipctl snip 200 200 600 300
snipctl key r
snipctl drag 100 80 500 220
snipctl key c
snipctl drag 0 0 300 300
snipctl key mod+z
snipctl key mod+shift+z
snipctl shot result
snipctl key mod+c
snipctl clipboard copied
snipctl save saved
snipctl compare copied saved
snipctl inspect saved
snipctl close
snipctl cleanup
```

Completion: capture shows the blue/green fixture; the rectangle appears; crop
keeps the blue region; undo restores the full image and redo restores the crop;
the final PNG is 300×300 with the visible rectangle portion; Copy and Save have
identical decoded pixels. `mod` means Ctrl on Windows and Cmd on macOS.

## Coordinates and checks

- `snip` uses screen units: physical pixels on Windows, points on macOS.
- `drag`, `click`, and `double-click` use source-image pixels at the current
  painted zoom/pan. Use `layout` to see the transform. Target visible points;
  use `key mod+0` to bring the full image back into view.
- Use `*-screen` commands for toolbar controls or absolute desktop positions,
  derived from a current screenshot. Coordinates vary with wrapping and scaling.
- `snip` prints the actual source-image dimensions reported by the painted editor.
  This report locates input; verify correctness through the visible result and
  the exported image, rather than treating diagnostics as proof.
- `compare A B` accepts run artifact names or PNG paths and fails on changed
  dimensions or RGBA pixels. It ignores PNG encoding and metadata differences.
- `inspect` reports dimensions and diagnostic exact-color counts. Judge content
  by geometry and appearance; RGB totals vary with scaling, antialiasing, styles,
  and display profiles.

## Evidence and targeted checks

Keep a screenshot at meaningful checkpoints and on failures. Inspect the actual
clipboard/file output for export changes; status text alone is insufficient.
Compare Copy and Save once per flow. Add before/after exports when investigating
an unintended pixel change or proving an undo/redo restoration.

Use native scaled-display/Retina checks when changing capture coordinates,
viewport transforms, DPI, or monitor handling. Automated scale-change tests cover
routine work. Use the [Export](features/export.md) Paint recipe when changing
clipboard formats or lifetime. Hardware pinch needs a touchpad.

Report what was exercised, its result, and any relevant untested path with the
reason. A toolbar action proves that entry point; shortcut delivery needs its
own check when shortcuts are in scope.

Logs, screenshots, and exported PNGs live under the run directory. `steps.log`
records actions; `app.log` records app diagnostics. `cargo clean` removes them.

## Maintaining snipctl

OS adapters live in `snipctl/src/platform/`. `launch` enables `SNIP_SNIP_VERIFY`
so the editor logs its painted image transform and actual content size. This is
read-only observation; actions still use OS input and output checks use PNGs.
Keep the diagnostic format and reader in sync. Run snipctl tests, formatting,
and Clippy; check its macOS target when changing shared code or its macOS adapter.
