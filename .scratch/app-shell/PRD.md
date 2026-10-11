Status: implemented

# App shell

## Problem Statement

snip-snip runs in the background, but screenshot authors have no visible way
to find its controls, change the capture shortcut, or quit it. The fixed
shortcut can conflict with another app; if registration fails, the current
app exits instead of offering recovery.

## Solution

Give the background app a Windows tray icon or macOS menu-bar icon with
New snip, Settings, and Quit. A small Settings window lets users change the
global capture shortcut. Keep the existing capture, overlay, and editor
workflow, and keep the app available after its windows close. Report
configuration and OS failures visibly, with a route to correct them through
Settings.

## User Stories

1. As a screenshot author, I want a tray or menu-bar icon while snip-snip runs, so that I can find the background app.
2. As a Windows user, I want the tray icon to identify snip-snip, so that I can distinguish it from other background utilities.
3. As a macOS user, I want a readable menu-bar icon in light and dark appearances, so that I can find the app in either appearance.
4. As a screenshot author, I want the icon's menu to offer New snip, Settings, and Quit, so that I can discover the app's background controls.
5. As a screenshot author, I want New snip to start the usual frozen-screen selection, so that I can capture without memorizing a shortcut.
6. As a screenshot author, I want the menu to dismiss before capture, so that its controls do not appear in my screenshot.
7. As a screenshot author, I want repeated capture requests during selection to leave that selection intact, so that I do not get overlapping capture overlays.
8. As a screenshot author, I want Settings to open from the icon's menu, so that I can configure the background app.
9. As a screenshot author, I want reopening Settings to focus its existing window, so that I do not get duplicate settings windows.
10. As a screenshot author, I want Settings to show my applied capture shortcut, so that I know how to start a snip.
11. As a new user, I want Alt+Shift+S as the initial capture shortcut, so that the existing documented shortcut continues to work.
12. As a screenshot author, I want to record a replacement shortcut using my keyboard, so that I do not need to learn a configuration syntax.
13. As a screenshot author, I want the proposed shortcut displayed before applying it, so that I can check what I recorded.
14. As a screenshot author, I want Escape to cancel shortcut recording, so that I can abandon a mistaken change.
15. As a screenshot author, I want shortcut recording to avoid starting a snip, so that trying a chord does not interrupt Settings.
16. As a screenshot author, I want unsupported or incomplete shortcuts rejected clearly, so that I understand why a change cannot apply.
17. As a screenshot author, I want a shortcut conflict explained visibly, so that I can choose another chord.
18. As a screenshot author, I want my previous shortcut to keep working when a replacement fails, so that I can still capture.
19. As a screenshot author, I want an applied replacement to take effect without restarting, so that I can use it immediately.
20. As a screenshot author, I want the previous shortcut released after a successful replacement, so that it is available to other apps.
21. As a screenshot author, I want my applied shortcut remembered across restarts, so that I configure it only once.
22. As a screenshot author, I want failure to save a shortcut reported rather than claimed as success, so that I can trust the applied value.
23. As a screenshot author, I want Settings and New snip available when startup shortcut registration fails, so that I can recover without editing files.
24. As a new user, I want missing settings to use sensible defaults, so that my first launch requires no setup.
25. As a screenshot author, I want unreadable or malformed settings explained visibly with usable defaults, so that a configuration problem does not make the app inaccessible.
26. As a screenshot author, I want closing Settings to leave the app running, so that I can continue taking screenshots.
27. As a screenshot author, I want canceling a capture or closing an editor to leave the icon and shortcut available, so that I can start another snip.
28. As a macOS user, I want closing the last app window to restore focus as it does today, so that I can return to my previous app.
29. As a screenshot author, I want Quit to exit the background app and remove its runtime icon and shortcut, so that it stops when I ask.
30. As a screenshot author, I want settings changes to preserve open editors, so that configuring the shell does not discard my annotations.
31. As a screenshot author, I want Copy & Close to close the editor while the shell stays available, so that I can quickly take another screenshot when Save flow is combined with this feature.

## Implementation Decisions

- **Architecture and ownership.** Respect the accepted GPUI architecture
  ADR. Retain the Rust UI, pinned framework, explicit-quit lifecycle, and
  existing capture/overlay entry point. Add focused shell, Settings, and
  native integration modules; keep startup wiring small. The shell owns its
  icon, menu, registered shortcut, and event delivery for the app lifetime.
  No document schema, renderer, or editor export changes are required.
- **Native menu.** Use native tray/menu-bar facilities with an identifiable
  snip-snip icon and tooltip or equivalent accessible name. The menu contains
  New snip, Settings, and Quit. Make the macOS icon suitable for both menu-bar
  appearances. Forward native events into GPUI's app context without idle
  polling. Dismiss the menu before capturing so it is absent from the frozen
  screenshot; verify that ordering natively.
- **Settings interaction.** Open and activate one small Settings window;
  later requests focus that window. Show the applied shortcut, a Record
  shortcut control, the proposed chord, and an Apply control. Escape cancels
  recording; closing Settings discards unapplied shortcut changes. Keep
  labels, applied values, and errors visible
  and usable with keyboard navigation. Settings does not change open editors.
- **Shortcut recording and validation.** Preserve Alt+Shift+S as the initial
  shortcut on both platforms. Record one supported nonmodifier key with at
  least one of Control, Alt/Option, Command/Super, or their combinations;
  Shift may supplement those modifiers. Reject modifier-only input, bare
  keys, Shift-only chords, and Escape as the capture key. Display modifier
  names using the current platform's conventions. Suppress shell capture
  dispatch while recording and cancel recording on focus loss. Unsupported
  and OS-reserved chords receive an actionable error rather than a silent
  fallback.
- **Shortcut replacement.** Apply a candidate only if native registration
  and durable persistence succeed. Preserve the previously applied shortcut
  and saved value on failure; avoid unregistering the working shortcut merely
  to test a candidate. Release the old shortcut after successful replacement.
  Treat reapplying an unchanged chord as a no-op. Dispatch only pressed events
  for the currently applied registration, so stale queued events cannot start
  a capture after replacement. Retain the overlay's guard against a second
  capture during an active selection.
- **Persistence.** Store only the capture-shortcut configuration in a
  per-user app settings location. Use a stable, validated representation and
  replace the settings file atomically so partial writes do not corrupt it.
  Missing settings use defaults without a warning. Unreadable or malformed
  settings use defaults for the current session with a visible explanation;
  do not silently overwrite the problem file on startup. An explicit
  successful Apply may repair it. Tests use isolated temporary storage.
- **Startup recovery.** Create a reachable shell even if shortcut
  registration or settings loading fails. Show an actionable Settings error
  on this exceptional startup; normal launches open no windows. New snip
  remains usable without a registered shortcut. If the default or saved chord
  is unavailable, do not silently choose another chord or terminate the app.
  An unrecoverable shell/icon initialization failure must be reported visibly
  before exit rather than leaving an inaccessible background process.
- **Lifecycle.** Keep the shell running when Settings, overlays, or editors
  close. Preserve existing native-close handling and last-window macOS focus
  restoration from the ADR. Explicit Quit closes app windows, releases native
  runtime resources, and exits without changing saved preferences. It follows
  existing window-close behavior;
  adding unsaved-editor confirmation is outside this slice.
- **Platform dependencies.** Select the smallest native integration that
  works with the pinned GPUI event loops on Windows and macOS. Validate that
  compatibility before committing to a library. Do not upgrade GPUI or add a
  second UI framework to deliver the shell.
- **Keyboard layout safety.** The native macOS hotkey backend uses physical
  U.S. key positions while GPUI supplies layout-derived characters. Support
  ABC and U.S. layouts in this slice; reject unsupported layouts visibly
  rather than reporting a successful binding to the wrong key. A layout
  change cancels recording and disables an unsupported binding. New snip
  remains available; returning to a supported layout attempts registration
  again. Broader layout translation requires native macOS verification.
- **Parallel integration.** App shell owns startup, native shell controls,
  hotkey management, and its Settings window. The Save
  flow thread owns editor exports, save/copy actions, and editor tests. Build
  the shell without depending on Save flow changes. Merge shared manifests
  and documentation deliberately. After combining the slices, verify that
  Copy & Close leaves the shell running and a second capture succeeds.
- **Documentation.** Update app usage and feature verification guidance for
  the menu, shortcut replacement, errors, and Quit. Update the
  roadmap after implementation and record remaining native checks accurately.

## Testing Decisions

- **Agreed primary seam.** Kevin confirmed reusing GPUI's app and
  visual test contexts to drive the rendered Settings window through normal
  mouse and keyboard input. Feed menu selections and hotkey presses through
  the shell's normal event-delivery boundary. Introduce one narrow shell OS
  adapter for native menu/hotkey events, hotkey registration, and controlled
  failures. Prefer real temporary settings files over mocking
  persistence. This is a shell-level extension of the existing harness, not
  a separate framework or an API for inspecting private Settings state.
- **Good tests.** Assert visible applied values, proposed chords, errors,
  window creation/closure, emitted capture requests, persisted contents after
  restart, and externally observable OS effects. Do not call private handlers,
  inspect transient fields, or prescribe internal call sequences. A fake OS
  adapter proves shell behavior under failure; it does not prove native icon,
  menu, or shortcut integration.
- **Prior art and modules.** Existing editor tests drive rendered controls
  and keyboard input with GPUI's visual harness, while the native snipctl
  workflow drives the actual shortcut, frozen overlay, editor, and file
  picker. Extend those approaches for the shell and Settings. Core document
  and rendering modules are unaffected and need no new tests for this slice.
- **Settings and persistence.** Cover first launch, recording/cancellation,
  validation, successful replacement, release of the old chord, restart with
  the saved chord, unchanged Apply, and visible recovery from missing,
  malformed, or unreadable configuration. Exercise registration conflicts
  and persistence failures through Settings, proving the prior chord remains
  usable and the applied display does not claim a failed change. Combine
  related assertions into focused scenarios.
- **Shell events and lifecycle.** Cover menu capture without a shortcut,
  single-instance Settings focus, startup hotkey failure recovery, stale and
  released hotkey events, recording suppression, repeated capture requests,
  all-windows-closed survival, and explicit Quit. Observe app windows and
  capture dispatch at their external boundaries rather than reading shell
  bookkeeping. Check Settings layout and keyboard usability in the rendered
  window.
- **Native proof.** On Windows and macOS, inspect icon legibility, menu
  interaction and dismissal before capture, actual global shortcut
  replacement/conflicts, recovery, Settings activation, focus return, and
  resource cleanup on Quit. Normal launch must open no windows. Extend
  snipctl only where its existing commands cannot drive those user paths.
  Do not treat simulated events as proof of native OS integration.
- **Parallel verification.** Independent worktrees can build and run
  automated tests concurrently. Native verification shares the desktop and
  machine-wide hotkeys, so run one app session at a time and coordinate with
  the Save flow thread. Do not stop an instance started by someone else to
  free its shortcut. After integrating both branches, drive capture to editor
  to Copy & Close to another capture and preserve macOS focus restoration.
- **Required checks.** Implementation requires workspace tests, all-target
  Clippy with warnings denied, formatting, and diff checks. Record unavailable
  platforms or hardware as outstanding. PRD publication
  alone does not require building or testing the application.

## Out of Scope

- Editor annotation, selection, crop, navigation, undo/redo, save, clipboard,
  default folders or filenames, and Copy & Close implementation.
- Additional preferences, a general settings framework, configurable menus,
  multiple global shortcuts, capture modes, history, or account sync.
- Linux support, a GPUI upgrade, a renderer redesign, or another UI framework.
- Installers, signing, notarization, distribution, automatic updates, or a
  general service-management system.
- Silently changing a conflicting shortcut or taking another app's shortcut.
- Process-wide single-instance IPC and unsaved-editor confirmation on Quit.
- Completing all earlier roadmap slices' native checks before this work.

## Further Notes

This is MVP roadmap slice 8. The app already has a global shortcut, an
active-selection guard, an explicit-quit lifecycle, and macOS focus
restoration. Extend those behaviors rather than replacing the capture flow.

App shell can proceed alongside the referenced Save flow thread in separate
worktrees. Shared manifests, the roadmap, README, and verification guidance
are expected merge points. Native desktop verification requires coordination
even though source implementation is independent.

The three menu commands, one Settings window, shortcut recorder with explicit
Apply, and initial Alt+Shift+S are synthesized defaults from the conversation
and existing app behavior. No product interview is needed for this spec.
Native library and platform registration details remain implementation
choices subject to the behavior and architecture above.

## Verification

- Workspace tests pass: 80 scenarios (22 core, 46 editor, 12 shell). Settings
  tests use rendered input, normal shell events, the narrow native adapter,
  and real temporary files. The same-chord layout-return scenario runs 32
  scheduling seeds. The Unix unreadable-file permissions scenario is present
  but could not run on this Windows host.
- Workspace all-target Clippy with warnings denied, formatting, diff checks,
  and the Windows release build pass. The separate snipctl project also passes
  all-target Clippy and formatting.
- Native Windows evidence is retained under `target/verify/`: run
  `1791748148` proves shortcut recording/replacement, real hotkey capture,
  editor closure, and a 600×300 exported fixture with 90,000 blue and 90,000
  green pixels; `1791748193` proves restart persistence and windowless launch;
  `1791758492` proves visible handling of a chord intercepted by another app.
  Final run `1791760374` proves two menu captures with neither the menu nor
  Explorer overflow flyout in the frozen image, followed by clean native Quit.
  Run `1791760250` shows the rendered Settings window with the saved shortcut.
  All owned sessions are stopped and the original user preference restored.
- macOS native icon/menu, hotkey/layout changes, light/dark appearance, focus
  return, and resource cleanup remain unverified here. Shortcuts explicitly
  support ABC and U.S. layouts; other macOS layouts are rejected visibly.
  Copy & Close followed by another capture remains a combined Save flow/App
  shell check after the branches are integrated.

## Comments

Kevin confirmed the testing seams: drive the rendered Settings window and
shell menu/hotkey events with the existing GPUI harness, use one narrow shell
OS adapter for controlled registration failures and isolated settings storage,
and verify the actual tray/menu bar and global shortcut through native
Windows/macOS checks.

Implementation reviews compared the working tree and new files against
`81af7ca5ce49aea166c4056518ea79b43ff78c65`. Findings on macOS key mapping,
unreadable configuration repair, and two Apply/layout interleavings were
fixed. Final Standards and Spec reviews report zero unresolved findings.
Windows native verification also exposed menu-loop dismissal ordering and
transient file-replacement contention; both were fixed and checked again.

The verification helper now provides `snipctl verify-shell [CHORD]` on
Windows. It owns a complete scenario, reports automated PASS/FAIL separately
from screenshot review, retains both launch logs, and restores original
preferences after success or a scenario failure. Native run `1791762374`
passed in 12.5 seconds with the expected fixture pixels and clean menu
capture; an earlier failed run proved process/preference cleanup. Four
focused helper tests, all-target Clippy, formatting, diff checks, and macOS
cross-compilation pass. The command's native scenario remains Windows-only.
