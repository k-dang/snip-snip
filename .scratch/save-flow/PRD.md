Status: implemented

# Save flow

## Problem Statement

Screenshot authors have finished annotating or cropping, but exporting still
requires unnecessary decisions and cleanup. Save starts in the home directory
with a filename containing Unix seconds, which is difficult to recognize.
Copy leaves the editor open, requiring a separate close action before returning
to the task that prompted the screenshot.

Export failures must leave the screenshot available for retry. A convenient
completion action must never close the editor when the clipboard write fails.

## Solution

Keep the existing Save dialog, but start it in a dedicated screenshot folder
with a readable timestamped PNG filename. Keep Copy for users who want to
continue editing, and add Copy & Close for users who are done. Close only after
a successful clipboard write. Present actionable export errors in the editor.

This is MVP roadmap slice 7, targeting Windows and macOS.

## User Stories

1. As a screenshot author, I want Save to start in a sensible screenshot folder, so that I do not have to navigate from my home directory for every export.
2. As a Windows user, I want the default folder to follow my platform's Pictures location, so that redirected or relocated user folders work normally.
3. As a macOS user, I want screenshots grouped under Pictures, so that saved captures are easy to find.
4. As a screenshot author, I want the default screenshot subfolder created when I first save, so that I do not need to set it up myself.
5. As a screenshot author, I want a safe fallback when Pictures is unavailable, so that saving can still start in a known location.
6. As a screenshot author, I want a readable date and time in the suggested filename, so that I can identify when I exported a capture.
7. As a screenshot author, I want filenames that are valid on both supported platforms, so that sharing an image does not require renaming it.
8. As a screenshot author, I want repeated exports to suggest distinct names, so that an earlier screenshot is not accidentally replaced.
9. As a screenshot author, I want to change the suggested name in the save dialog, so that I can give an image a meaningful title.
10. As a screenshot author, I want to choose a different destination in the save dialog, so that I can save directly into a project or shared folder.
11. As a screenshot author, I want to cancel the save dialog without an error, so that deciding against an export does not interrupt editing.
12. As a screenshot author, I want Save to keep the editor open, so that I can continue editing or copy the same image afterward.
13. As a screenshot author, I want confirmation of the saved path, so that I know where the image went.
14. As a screenshot author, I want a visible error if the default folder cannot be prepared, so that I know why saving did not proceed.
15. As a screenshot author, I want a visible error if the save dialog or file write fails, so that a failed export is not mistaken for success.
16. As a screenshot author, I want my document to remain available after an export failure, so that I can retry without recapturing or recreating annotations.
17. As a screenshot author, I want Copy to keep the editor open, so that I can paste a version and continue refining it.
18. As a screenshot author, I want a clearly labeled Copy & Close control, so that I can finish a capture in one action.
19. As a keyboard user, I want a Copy & Close shortcut, so that I can finish without moving to the toolbar.
20. As a screenshot author, I want Copy & Close to close only after clipboard success, so that a failed copy does not discard my editing session.
21. As a screenshot author, I want Copy & Close failures to appear in the editor, so that I can retry Copy or save instead.
22. As a screenshot author, I want copied output to remain available after the editor closes, so that I can paste it into another application.
23. As a screenshot author, I want finishing one editor to leave other editors and the background app available, so that I can continue another capture or start a new one.
24. As a macOS user, I want closing the last editor to return me to the previous application, so that Copy & Close fits the existing background workflow.
25. As a screenshot author, I want exports to include the text I am currently editing, so that my last label changes are not lost.
26. As a screenshot author, I want export actions during an unfinished drag to follow existing cancellation behavior, so that partial annotations or crop adjustments do not enter the output unexpectedly.
27. As a screenshot author, I want all three export actions to use the same document rendering and crop, so that saving and copying produce the same image.
28. As a screenshot author, I want output at the original screenshot resolution regardless of zoom or pan, so that navigating the editor does not change image quality or framing.
29. As a screenshot author, I want editor controls and selection overlays excluded from export, so that recipients see only the screenshot and annotations.
30. As a screenshot author, I want the export controls reachable at the minimum editor size, so that the new completion action does not hide existing tools.
31. As a Windows or macOS user, I want existing Copy and Save shortcuts to retain their behavior, so that completing a capture does not require relearning the editor.

## Implementation Decisions

- **Architecture.** Follow the accepted GPUI architecture ADR. Extend the
  editor's export actions, toolbar, keyboard dispatch, and status feedback.
  Retain the core document, renderer, PNG encoding, and existing export
  preparation: cancel an unfinished gesture, finish text through its normal
  history path, and render the committed document with its crop. Export does
  not depend on the viewport. No document schema or history changes are needed.
- **Default folder.** Resolve the platform's user Pictures directory and use
  its `snip-snip` subfolder. Respect platform redirection rather than assuming
  Pictures is a literal child of the home directory. If Pictures cannot be
  resolved, use a `snip-snip` subfolder of the resolved home directory. If
  neither location can be resolved, show a Save failure. Prepare the directory
  when Save is invoked, not at app startup. A creation or access failure shows
  an error and leaves the editor open; do not silently save into the process
  working directory or switch destinations after an I/O failure.
- **Filename.** At each Save invocation, suggest a local-time filename in the
  form `snip-YYYY-MM-DD_HH-mm-ss-SSS.png`, with milliseconds and zero-padded
  fields. The timestamp denotes export time. If that name already exists in
  the default folder, suggest the first available numeric suffix, starting
  with `-2` before `.png`. Generate the suggestion without reserving an empty
  file. A suggestion is not an overwrite guarantee: the native picker remains
  responsible for confirming replacement of an existing user-selected file.
  Surface time or directory-enumeration errors rather than inventing a
  misleading date or ignoring an I/O failure.
- **Save dialog and write.** Preserve Save… and Ctrl/Cmd+S as dialog-based
  export. Supply the default folder and suggested filename on every invocation;
  let the user edit both through the native picker. Keep output as PNG. Cancel
  is a normal outcome, while picker errors and write errors receive explicit
  status messages. Encode and write the exported snapshot off the UI thread.
  Report success only after writing finishes, including the destination path.
  Save keeps the editor open and adds no document history entry of its own.
  If the editor closes while a save is completing, the accepted write may
  finish without trying to reopen a window.
- **Copy and Copy & Close.** Keep Copy and Ctrl/Cmd+C as copy-only actions.
  Add a separate Copy & Close toolbar control and Ctrl/Cmd+Shift+C shortcut.
  Dispatch modified shortcuts explicitly so Shift+C reaches the new action
  rather than the existing Copy branch. Both actions share the same clipboard
  write and exported pixels, retaining arboard and its Windows image-format
  compatibility. Copy reports success or failure in the status area. Copy &
  Close removes its own editor window only after the write returns success;
  a failure reports the error and keeps the editor open for retry. Follow the
  existing window-removal and last-window focus behavior. Keep the background
  app running and other editor windows intact.
- **Failure and editing behavior.** Export errors do not remove annotations,
  clear the crop, or reset the viewport. Preserve existing text finalization
  and gesture cancellation even when the subsequent OS operation fails.
  A retry exports the document as it exists at that retry. Success and error
  status are visible through the existing status area. Do not add a modal
  confirmation before Copy & Close.
- **Testable OS effects.** Keep one rendered-editor test entry point. Introduce
  only the small effect boundary needed to supply a test destination/time and
  to observe or fail a clipboard write deterministically. Share the production
  copy path between Copy and Copy & Close; the adapter substitutes the OS
  effect, not editor logic. Use real temporary files for save assertions and
  the existing GPUI picker simulation. Do not expose private document or
  action handlers as a testing API or build a generic service framework.
- **Layout and documentation.** Fit the additional control into the existing
  wrapping toolbar, keeping all export actions usable at the minimum window
  size. Document the default folder, filename convention, shortcut, and
  success-only close behavior. Mark the roadmap slice implemented only after
  implementation and verification, recording outstanding native checks.

## Testing Decisions

- **Confirmed primary seam.** Kevin confirmed the existing rendered-editor
  GPUI harness: drive toolbar clicks and normal keyboard input, inspect saved
  PNGs, visible status, and window closure, and control clipboard success or
  failure through a small OS-effect adapter. Supplement this with native
  Windows/macOS save-picker and clipboard-paste checks.
- **Good tests.** Check externally observable behavior and user outcomes.
  Observe picker defaults, files and their decoded pixels, clipboard payloads,
  status, and the number of open windows. Avoid private editor state, calls to
  private action handlers, and tests that merely repeat filename or rendering
  implementation arithmetic. Use fixed timestamps and explicit expected names.
- **Prior art and modules.** Extend the editor's existing rendered-event,
  simulated-save-picker, saved-PNG, text-finalization, crop, and window-close
  scenarios. The current harness already drives platform-specific shortcuts
  and compares actual PNGs with expected core rendering. Retain core rendering
  and history tests; add core coverage only if those contracts actually change.
- **Folder and naming.** Drive Save against controlled Pictures/home locations
  and a fixed local timestamp. Observe default folder creation and picker
  suggestions, including home fallback and unavailable locations. Prepopulate
  colliding names and check the next suggestion without changing their bytes.
  Cover a timestamp near a date boundary and ensure the generated name is valid
  on Windows. Keep filesystem effects confined to temporary directories.
- **Save outcomes.** Choose a custom destination through the simulated picker
  and compare the resulting PNG to the expected annotated, cropped document.
  Verify success status and continued editing. Cover cancellation, directory
  preparation failure, picker failure, and a deterministic file-write failure
  such as choosing a directory as the file path. Observe failure status and
  successful retry; avoid permission-based cases whose behavior depends on
  elevated test privileges.
- **Copy outcomes and closure.** Through toolbar clicks and shortcuts, observe
  identical image payloads for Copy and Copy & Close. Assert Copy leaves the
  window open, successful Copy & Close removes only its own window, and a
  clipboard failure leaves the editor open with an error. Retry after failure.
  Cover closing one of two editors and using the global capture workflow again
  after closing the last editor in native verification.
- **Editing integration.** Use a focused fixture with edited text and a crop
  to prove both saved and copied output include finalized text, respect the
  crop, and exclude overlays. Exercise an unfinished drag to confirm existing
  cancellation semantics. Reuse existing zoom/export coverage rather than
  duplicating every viewport case. Check that the new modified shortcut works
  while text editing, without typing an extra character or invoking Copy-only.
- **Layout and native verification.** Extend minimum-window toolbar coverage
  to the new control. On Windows and macOS, verify the actual Pictures location,
  folder creation, picker defaults, custom filename/destination, cancellation,
  and copy-and-close from a capture. Paste into another application after
  closure and compare dimensions and visible content. Clipboard readback alone
  does not establish application-paste compatibility. Record unavailable
  platforms as outstanding rather than treating simulations as native proof.
- **Required checks.** Implementation requires workspace tests, workspace
  Clippy with warnings denied, formatting, and diff checks. This spec-only
  publication requires Markdown review and diff checks, not an application build.

## Out of Scope

- Direct save without a picker, Save & Close, automatic saving, a screenshot
  library, or export formats beyond the existing PNG workflow.
- A folder-preference UI, remembering the last chosen directory, persistent
  export settings, configurable filename templates, or configurable shortcuts.
- Tray/menu-bar integration, launch at login, and global hotkey configuration
  from roadmap slice 8.
- Renderer changes, image resampling, document persistence, unsaved-change
  prompts, or changes to existing text and gesture history semantics.
- Linux support, framework upgrades, and a new testing framework.
- Completing all prior slices' outstanding native checks as a prerequisite.

## Further Notes

The roadmap requests a default folder, a timestamped filename, and copy and
close in one action. Existing Save already suggests a Unix-seconds filename
and opens the native picker in the home directory; existing Copy keeps the
editor open. This slice improves those behaviors without replacing export.

The Pictures subfolder, home fallback, readable export-time format, collision
suffix, retention of the save dialog, and separate Copy & Close shortcut are
concrete defaults synthesized for this spec, not preferences separately stated
by Kevin. Kevin explicitly confirmed the testing seams.

## Verification

Implemented through the confirmed rendered-editor seam. Six focused export
scenarios cover default folder creation, redirected Pictures and home fallback,
unavailable directories and time, midnight naming, collision suffixes, normal
cancellation, folder/picker/write failures and retries, text finalization and
history, cropped output, both copy actions, clipboard failure, successful
closure, unfinished-drag cancellation, and multiple editors. Existing toolbar
coverage includes Copy & Close at both tested display scale factors. Save-folder
preparation runs on the background executor. A rendered-input regression proves
that Save returns before folder access, Copy remains available while preparation
is pending, switching editors retains picker ownership with the initiating
editor, and closing that editor prevents a delayed picker.

Checks passed:

- Workspace tests: 74 total (22 core, 52 rendered-editor).
- All-target editor typecheck, workspace Clippy with warnings denied,
  formatting, and diff checks.
- Windows release build.
- Verification CLI Windows build and Clippy with warnings denied, formatting,
  and macOS ARM64 cross-target typecheck.

Native Windows verification used the repository's snipctl workflow. Read-only
native dialog inspection confirmed the redirected Pictures destination and a
readable local timestamp. The final release run captured a fixture, annotated
it with a rectangle and text, canceled the native picker while retaining the
editor, saved under a custom name in the evidence directory, copied, and used
the Copy & Close toolbar control. Saved, copied, and post-closure clipboard
readback images have identical decoded pixels at 600 × 300. The app remained
healthy without an editor, and a subsequent capture worked. Verification app
and fixture processes were cleaned up.

The CLI gained a cancel-save command because its generic key command focuses
the editor: sending Escape that way closes the Windows editor instead of
targeting its separate save dialog. The new command focuses and dismisses the
actual dialog and asserts the editor remains open. Its Windows behavior was
verified natively; its macOS implementation was typechecked only.

Save-flow release evidence is retained under `target/verify/1791739885/`, including
screenshots, exported images, decoded-pixel comparison, and action/app logs.
Picker-default inspection is under
`target/verify/1791739338/save-dialog-defaults.txt`.
These native runs precede the background-preparation review fix. A follow-up
Windows release run under `target/verify/1791746852/` verified native Save and
cancellation with the fix: clipboard readback before saving and the saved PNG
both measured 600 × 300 with 90,000 blue and 90,000 green fixture pixels. App
health checks passed and verification processes were cleaned up. An earlier
follow-up capture under `target/verify/1791746795/` unexpectedly measured
101 × 52; the repeat verified the capture before Save and matched the expected
fixture. Pending input, queued activation, picker ownership, and window closure
were verified through the rendered-editor seam.

Ctrl+Shift+C did not reach the native editor on this machine. A Windows hotkey
registration probe returned error 1409, confirming the chord is already
reserved globally; another Ctrl+Shift chord reached the editor normally.
No system hotkeys were changed. Both toolbar and shortcut completion pass
automated tests, and toolbar completion was verified natively using logged
screenshot-derived click coordinates. Native Ctrl+Shift+C delivery on a
machine without that reservation remains outstanding, along with macOS and
pasting clipboard output into another application. Clipboard readback proves
payload persistence, not application-paste compatibility. Native export
failure injection remains covered by the deterministic automated seam.

## Comments

Independent Standards and Spec reviews compared the implementation and new
modules against the preimplementation commit. Standards reported zero findings.
Spec reported zero code findings and requested the roadmap completion update,
which was applied with the native verification gaps. Both reviewers rechecked
the final verification-helper changes and documentation and reported zero
remaining findings.

A subsequent GPT-6.1 Sol review identified synchronous save-folder preparation
on the UI thread. Folder creation, enumeration, and collision checks now run
on the background executor. Its follow-up review caught native Windows' queued
activation: the picker now waits for that foreground turn and validates its
initiating window before opening. The regression models queued OS activation
and records the active owner at prompt invocation. Both failures were observed
before their fixes. The reviewer rechecked and cleared both findings with no
remaining regression in the ownership and lifecycle changes.

The maintainability review's two follow-up findings were addressed. The effect
boundary now exposes one awaitable, window-owned chooser; activation ordering,
owner validation, window closure, and picker-receiver handling live in its
shared implementation. The existing rendered-editor regression still uses
that production protocol with queued OS activation. Save handles only the
chosen path, cancellation, or failure, with one final status match.

The macOS verification CLI now polls the native Accessibility dialog and its
focus before sending input, and waits for dismissal after cancellation. Save
shares the readiness check. A readiness timeout returns before sending Escape.
Both platforms establish dismissal before the shared editor-preservation
assertion, replacing its extra fixed delay. Workspace tests (74), workspace
Clippy, Windows and macOS ARM64 verification-CLI checks/Clippy, formatting,
diff checks, and the Windows release build passed. Native Windows Save and
cancellation passed again under `target/verify/1791760159/`; copied and saved
fixtures both measured 600 × 300 with 90,000 blue and 90,000 green pixels.
Verification processes were cleaned up. The macOS Accessibility script was
cross-typechecked only and still needs execution on a Mac with its required
permissions.
