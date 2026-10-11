# App shell

snip-snip stays available through its tray/menu-bar icon. New snip starts
capture, Settings records and applies a persistent global shortcut, and Quit
exits the app. Closing an ordinary window keeps the shell running.

## Sub-features

- New snip, Settings, and Quit in the native menu.
- One Settings window; reopen to focus it.
- Shortcut recording, validation, cancellation, replacement, and persistence.
- Visible registration/configuration failures with the previous chord retained.
- Capture after all windows close and cleanup on explicit Quit.

## How to get to it (user POV)

Open the snip-snip tray/menu-bar icon. Choose Settings, Record shortcut, press
a chord, and Apply. Escape cancels recording; Tab and Enter operate the two
controls. Close Settings and capture with the new chord or the menu.

## Driving it with snipctl

On Windows, build the current release binary, coordinate with other threads,
and announce that the run uses real mouse/keyboard input. Start with no
snip-snip running and an unlocked desktop; the 800×500 fixture at 100,100 must
fit on the primary monitor. Run:

```sh
snipctl verify-shell
```

The command owns its run: windowless launch, opening Settings twice and
checking its identity/focus, recording and applying a replacement, capture
with that chord after Settings closes, fixture export, editor closure,
native Quit, restart and capture with the saved chord, menu capture and
cancellation, and final Quit. It asserts the exported fixture is exactly
600×300 with its expected blue and green halves (90,000 pixels each).

The default test chord is `ctrl+alt+k`. If it is unavailable, use an available
alternative such as `snipctl verify-shell ctrl+alt+l`. The command prepares an
Alt+Shift+S baseline while retaining the original preference file beside it,
then restores the original bytes and metadata on success or a scenario
error. Originally missing settings remain missing. It stops only its own
app/fixture processes. A failed check returns nonzero, retains logs and a
failure screenshot when possible, and reports any cleanup failure and backup
location. The backup location is also recorded in `steps.log`; after an
interrupted process, run `snipctl cleanup` before restoring that original.

Completion requires both the command's automated PASS and review of its
reported evidence directory:

- `menu-overlay.png`: no tray menu, selected menu-row remnant, or overflow
  flyout in the frozen image. Live menu closure alone does not prove this.
- `settings-applied.png`: the expected applied chord and readable labels,
  values, and errors. This image captures the Settings window directly.
- `fixture.png` is checked automatically; `fixture-editor.png` provides
  visual context. `app-first-launch.log`, `app.log`, and `steps.log` retain
  both launch histories and the checks.

Conflict/error behavior remains a separate focused native check: with an
owned session and preserved preferences, try a known-conflicting chord
through Settings and capture the visible error while the applied chord
remains usable. Restore preferences after cleanup. OS-adapter tests cover
controlled registration/storage failures; the happy-path command does not
claim native proof of those failures or of icon disappearance/legibility.

## Gotchas

- `shell-menu` uses Windows accessibility selectors and actual mouse input;
  it can open the hidden-icons tray panel. It is Windows-only. Native macOS
  menu-bar and focus checks still require a macOS session.
- `shell-key` focuses Settings if it exists, otherwise sends keys to the
  current foreground window. Confirm which window is active before sending it.
- `shot` captures the primary screen when no editor is open. Verify Settings
  and tray controls are on that display before inspecting the artifact.
- The default `cancel-snip` command sends Alt+Shift+S. With a configured chord,
  open capture with that chord and cancel through `shell-key esc` instead.
- Configuration uses the platform's per-user app folder; tests use temporary
  storage. A real preference change must not leak into another verification run.
- macOS shortcuts currently support ABC and U.S. layouts. Verify that another
  layout visibly disables the shortcut while New snip remains available,
  then switch back and verify recovery. Do not treat simulated layout events
  as proof of native Carbon key mapping.
- Copy & Close integration is checked only after combining the Save flow and
  App shell branches; this recipe does not claim to prove that integration.
