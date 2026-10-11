Status: implemented

# Zoom and pan

## Problem Statement

The editor always centers the screenshot and scales it down to fit the
available canvas. Large captures become too small to inspect or annotate
precisely. Users cannot zoom into a detail, move across an enlarged capture,
or return deliberately to an overview or the screenshot's actual pixel size.

## Solution

Add viewport controls to the existing editor: Ctrl+scroll on Windows or
Cmd+scroll on macOS, trackpad pinch to zoom, and Space+left-drag to pan.
Zoom follows the pointer or pinch center so the detail being inspected stays
in place. Fit and 100% are available through shortcuts and compact toolbar
controls, alongside the current zoom percentage.

New editors retain the current fit behavior. Viewing changes preserve the
active tool, selection, annotations, crop, and document history. Drawing and
editing continue to target the correct screenshot pixels at every view.
Saved and copied images retain their original resolution, subject only to
the document's crop.

## User Stories

1. As a screenshot author, I want a new capture to fit in the editor, so that I can see the entire screenshot before annotating it.
2. As a Windows user, I want Ctrl+scroll to zoom, so that I can inspect details with my mouse wheel.
3. As a macOS user, I want Cmd+scroll to zoom, so that I can use the platform's command modifier.
4. As a trackpad user, I want pinch to zoom without holding a modifier, so that inspecting a capture feels natural.
5. As a screenshot author, I want wheel zoom to stay anchored at the pointer, so that the detail I am inspecting stays under my cursor.
6. As a trackpad user, I want pinch zoom to stay anchored at the gesture center, so that the detail I am inspecting stays in place.
7. As a screenshot author, I want both wheel notches and smooth scrolling to produce predictable zoom, so that my input device does not make the view jump unexpectedly.
8. As a screenshot author, I want bounded zoom, so that repeated input cannot make the view unusably tiny or large.
9. As a screenshot author, I want to see the current zoom percentage, so that I know how much the screenshot is magnified.
10. As a screenshot author, I want a 100% shortcut, so that I can inspect screenshot pixels at their actual display size.
11. As a screenshot author, I want a Fit shortcut, so that I can return to an overview in one action.
12. As a screenshot author, I want visible Fit and 100% controls, so that I can discover these actions without knowing the shortcuts.
13. As a screenshot author, I want Fit to center the whole screenshot, so that returning from a close-up restores a useful overview.
14. As a screenshot author, I want 100% to center the screenshot, so that resetting magnification has a predictable result.
15. As a screenshot author, I want to hold Space and drag to pan, so that I can move around an enlarged capture without changing tools.
16. As a screenshot author, I want the image to follow my pan drag, so that moving the view feels direct.
17. As a screenshot author, I want a hand cursor while Space is held and a grabbing cursor while panning, so that the editor shows what my drag will do.
18. As a screenshot author, I want releasing Space or the mouse button to finish panning, so that my next drag uses my annotation tool again.
19. As a screenshot author, I want panning to preserve my tool and selection, so that I can continue the same edit after navigating.
20. As a screenshot author, I want Escape to cancel an active pan without closing the editor, so that I can recover from an accidental drag.
21. As a screenshot author, I want pan bounds to keep the screenshot reachable, so that I cannot lose it off canvas.
22. As a screenshot author, I want a screenshot that fits along an axis to stay centered on that axis, so that navigating does not create unnecessary empty space.
23. As a screenshot author, I want pan tracking to continue outside the canvas until release, so that a drag cannot leave the editor stuck in pan mode.
24. As a screenshot author, I want losing window focus to clear held navigation keys, so that returning to the editor does not unexpectedly pan.
25. As a screenshot author, I want drawing after zooming and panning to land on the pixels I point at, so that annotations remain precise.
26. As a screenshot author, I want new annotations to retain their chosen image-space thickness and text size when I zoom, so that viewing a detail does not change my drawing style.
27. As a screenshot author, I want selection, moving, and shape resizing to work at the current view, so that existing marks remain editable when magnified.
28. As a screenshot author, I want selection handles to retain a usable on-screen size, so that they remain easy to grab at different zoom levels.
29. As a screenshot author, I want text clicks, the caret, and the IME candidate location to follow the view, so that text editing stays aligned with the label.
30. As a screenshot author, I want to zoom while editing text without ending the editing session, so that I can inspect a label and continue typing in one undo step.
31. As a screenshot author, I want Space to insert a space while editing a label, so that navigation shortcuts never interfere with normal typing.
32. As a screenshot author, I want crop dragging and its overlay to follow the current view, so that a crop still selects the intended screenshot pixels.
33. As a screenshot author, I want zoom and pan to leave undo and redo intact, so that navigation does not consume edit history or discard a redo opportunity.
34. As a screenshot author, I want saving or copying from a close-up to export the full document or its crop, so that my current view never truncates the output.
35. As a screenshot author, I want viewport controls and overlays excluded from export, so that saved and copied screenshots contain only my annotations.
36. As a screenshot author, I want Fit to adapt when I resize the window, so that the whole screenshot stays visible in overview mode.
37. As a screenshot author, I want a manually chosen view to survive window resizing, so that changing the window does not unexpectedly reset my zoom.
38. As a user with scaled or Retina displays, I want 100% to mean one screenshot pixel per physical display pixel, so that actual-size inspection is consistent across displays.
39. As a user moving an editor between displays, I want zoom and pointer alignment to stay correct after a DPI change, so that I can keep working without resetting the view.
40. As a screenshot author, I want all toolbar actions to remain reachable at the minimum editor size, so that viewport controls do not crowd out drawing, Copy, or Save.
41. As a screenshot author, I want zoom input during an annotation drag to leave that drag undisturbed, so that navigation cannot distort a mark in progress.
42. As a screenshot author, I want navigation input over the toolbar to leave the canvas unchanged, so that interacting with controls cannot accidentally move my view.

## Implementation Decisions

- **Architecture.** Keep the all-Rust GPUI editor and the accepted renderer
  separation. The document model, core renderer, and export contract remain
  the source of truth for screenshot pixels and annotations. Viewport state
  belongs to each editor window; it is not document content or history.
- **Existing transform.** Extend the editor's existing viewport, which maps
  image pixels to logical window coordinates through an origin and scale.
  Resolve one current transform for painting and input. All annotation,
  selection, crop, caret, and OS text-input geometry use that transform.
  Input between paint frames must observe the latest navigation change.
  Keep platform navigation handlers and controls in `editor/navigation.rs`
  and transform calculations in `editor/view.rs`. Derive manual view state
  once when committing a transform. Compare the painted transform with the
  last painted view to invalidate OS character coordinates after navigation,
  resize, or DPI changes.
- **Initial view and Fit.** New editors start in Fit. Fit uses the available
  canvas with the existing 16-logical-pixel padding, centers the screenshot,
  and caps magnification at 100%, preserving the existing behavior for small
  captures. It fits the original screenshot, including the dimmed area outside
  an active crop. Fit recalculates when canvas size or display scale changes.
- **Actual size.** Zoom percentage is physical display pixels per screenshot
  pixel. At 100%, one screenshot pixel occupies one physical display pixel;
  logical pixels per screenshot pixel therefore equal the reciprocal of the
  window's device scale. Show a rounded percentage, but retain precise zoom
  internally. Fit may show a fractional percentage for very large captures.
- **Manual zoom bounds.** Use a fixed maximum of 800%. The minimum is the
  smaller of 10% and the current fit percentage, so zooming can always return
  to the overview without jumping upward on a large capture. These are
  implementation defaults chosen for this slice, not user settings. Clamp
  results, ignore zero or invalid deltas, and keep the transform finite.
- **Zoom input.** Handle the pinned GPUI's scroll and pinch events on the
  canvas. Wheel zoom requires Ctrl on Windows or Cmd on macOS. Unmodified
  scrolling does not navigate in this slice. Pinch requires no modifier.
  Normalize line and pixel wheel deltas and use multiplicative zoom rather
  than an additive scale change. Start with approximately 10% per wheel
  notch; smooth deltas produce proportionate changes. Use GPUI's pinch delta
  semantics rather than treating pinch as wheel input. Consume handled zoom
  input once, avoiding duplicate handling of one physical gesture.
  On Windows, GPUI line deltas already include the OS's scroll-line setting;
  query that setting through the read-only native API to recover notch counts.
  The Windows-only dependency reuses the locked Windows API crate. Its one
  pointer call has a documented, local unsafe allowance; the default unsafe
  lint remains enabled elsewhere. If the query fails, use the standard
  three-line setting as a fallback.
- **Anchoring.** Before zooming, find the image pixel under the event position;
  change origin so that pixel remains under the same window position. Pan
  constraints may override this at an image edge or when an axis fits.
  Zooming works anywhere inside the canvas, including its padding. Input
  outside the canvas does not change the view.
- **Fit and 100% actions.** Use Ctrl+0 / Cmd+0 for Fit and Ctrl+1 / Cmd+1 for
  100%. Both center the screenshot and clear the pan offset. Fit re-enters
  automatic fitting; 100% and wheel/pinch zoom choose a manual view. Add
  compact Fit and 100% toolbar actions and a current percentage indicator.
  Labels or tooltips communicate shortcuts. Preserve the existing wrapping
  toolbar and ensure its controls remain usable at minimum window size.
- **Temporary pan.** Outside text editing, Space key-down makes a subsequent
  left drag anywhere on the canvas a pan instead of an annotation gesture.
  Track movement in logical window coordinates and preserve zoom. Show open
  and closed hand cursors while available and active. Reuse the existing
  window-level drag tracking so release outside the canvas ends the pan.
  Releasing Space or the left button ends the pan at its current position;
  the remainder of that mouse press never becomes an annotation drag.
- **Pan bounds.** For each axis, center the image when it fits inside the
  padded canvas. When larger, clamp its origin so the image covers the padded
  canvas along that axis; users can reach both image edges without dragging
  the screenshot entirely out of view. Apply the same constraints after
  zoom, window resize, and display-scale changes.
- **Cancellation and focus.** Escape during an active pan restores its
  starting view and consumes the key before the existing clear-selection or
  close behavior. Window deactivation ends pan at its current position and
  clears held Space. Repeated Space key-down does not restart a pan. Mouse
  release, key release, and focus loss cannot leave stale navigation state.
- **Gesture arbitration.** Do not start panning midway through drawing,
  moving, resizing, or cropping. Ignore wheel, pinch, Fit, and 100% changes
  while any pointer gesture is active, including pan. The next gesture uses
  the current held-key state. Navigation alone never clears selection or
  switches tools, commits history, or invalidates redo.
- **Text editing.** Space remains normal OS text input while a text session
  is active; Space-to-pan is unavailable until the session ends. Wheel,
  pinch, and Fit/100% actions may change the view without ending the session
  or committing composition. Route viewport toolbar actions so the existing
  generic toolbar mouse-down handler does not finish text for these actions.
  Caret clicks and IME bounds follow the updated transform, including while
  composing. Other toolbar actions retain their existing text behavior.
  Invalidate OS character coordinates after view changes so an open candidate
  panel receives its new position, including after resizing or DPI changes.
- **Annotation styling.** Continue creating widths and text sizes with the
  existing device-scale conversion into image pixels. Do not divide them
  by the current zoom. Zoom magnifies existing and new content consistently.
  Selection handles and hit tolerances retain their existing logical sizes.
- **Resize and DPI changes.** In a manual view, preserve the chosen physical
  zoom percentage and the image point at the canvas center, then apply pan
  bounds. Recompute the logical scale when moving between displays. Fit
  continues to recompute its view. Skip transformation work when the canvas
  has no usable area, then resolve the view when usable bounds return.
- **Painting and export.** Navigation transforms the existing preview texture
  and overlays without recompositing the document on the CPU or replacing
  its texture. Clip all canvas painting, including crop dimming, live ink,
  selection handles, and text overlays, to the canvas; zoomed content must
  not paint over the toolbar. Saving and copying continue through the core
  export path, independent of the viewport.
- **Documentation.** Update editor usage documentation with gestures,
  shortcuts, and the physical-pixel meaning of 100%. Update the roadmap's
  state after implementation and record native verification gaps accurately.

## Testing Decisions

- **Agreed primary seam.** Extend the existing GPUI rendered-editor harness.
  Kevin confirmed this seam: drive editor input, observe rendered placement
  and saved PNGs, and supplement with native Windows/macOS checks for real
  pinch and DPI behavior. No additional test API or framework is needed.
- **External behavior.** Good tests exercise user or OS input and assert what
  the editor displays, what text input reports to the OS, or what it saves.
  Do not inspect private viewport state, call gesture handlers directly, or
  test a helper merely by repeating its arithmetic.
- **Existing capabilities.** The harness already provides mouse and key
  input, rendered-element bounds, window resize, device-scale changes, and
  a simulated save picker. Its general input-event dispatch can deliver
  scroll, pinch, and key-up events through GPUI's normal event path. Extend
  the current harness rather than adding a second seam.
- **Independent expectations.** Existing pointer helpers assume auto-fit and
  need adjustment for navigation cases. Derive expected positions from
  fixture dimensions, rendered canvas bounds, and the specified user action
  sequence, rather than reading the editor's private transform. Use a fixture
  with identifiable image details where needed to observe actual painting.
  Saved pixels alone do not prove that the displayed image moved correctly.
- **Zoom behavior.** Cover wheel modifier gating on the current platform,
  line and pixel deltas, pinch input, pointer anchoring away from constrained
  edges, unchanged anchoring at a zoom limit, Fit/100% shortcuts, and input
  over the toolbar. Observe visible placement or interactions with known
  screenshot landmarks as well as exported results.
- **Pan lifecycle.** Drive held Space, left down, movement, key-up, mouse-up,
  Escape, and deactivation through the window event path. Check that pan
  moves the visible screenshot without adding a mark, Escape restores its
  start, release outside the canvas works, bounds keep edges reachable, and
  the next annotation drag uses the original tool.
- **Editing through the view.** At a non-default zoom and pan, draw and move
  annotations, resize a shape through its visible handle, place or re-edit
  text, and crop. Compare saved PNGs with the existing core-rendered expected
  documents. Combine related behaviors into focused scenarios instead of
  duplicating every tool at every possible zoom.
- **Text and composition.** Follow existing OS-input tests: input-handler
  calls stand in for the OS, with caret and candidate bounds observed through
  its public contract. Check that a space types normally, zoom preserves the
  session and composition, caret clicking still inserts at the intended
  character, and updated bounds align with the transformed label.
- **History and output.** Draw, undo, navigate, and redo to prove that viewing
  preserves redo. Save an unchanged document from different views and compare
  dimensions and pixels; also save a cropped document from a close-up. Use
  native clipboard paste checks for copy, as the current harness's dependable
  automated output seam is the saved PNG.
- **Layout and DPI.** Extend the existing minimum-window-size toolbar checks
  to the new controls. Exercise 1x and 2x device scales, fitted large captures,
  manual resize, and a scale change. Assert usable canvas space, nonoverlapping
  controls, correct 100% placement, and aligned editing. Native visual checks
  confirm clipping and handle/caret appearance on screen.
- **Native verification.** On Windows and macOS, run the snip-to-editor flow,
  wheel zoom, real trackpad pinch where hardware is available, Space-to-pan,
  text editing, save, and clipboard paste. Inspect Retina/scaled displays and
  moving a window between different DPI displays when available. Report
  unavailable hardware or platforms as outstanding checks rather than
  treating simulated input as native proof.
- **Required checks.** Run the existing workspace tests, lint checks, and
  formatting checks after implementation. Add a core test only if an actual
  change to core behavior requires one; this feature should ordinarily stay
  in the editor. No test run is required to publish this specification alone.

## Out of Scope

- Crop handles, clearing crops, and drag pixel-size labels; those belong to
  the following crop-polish slice.
- Changes to the save workflow, copy-and-close, the app shell, or global hotkey
  settings.
- Unmodified wheel/two-finger-scroll panning, middle-button panning, a
  permanent hand tool, inertial panning, scrollbars, or a minimap.
- Numeric zoom entry, configurable zoom limits or sensitivity, and persistent
  viewport preferences across editor windows.
- Image rotation, resampling the document to match the view, or changes to
  the core renderer's annotation geometry, fonts, or export resolution.
- A GPUI upgrade, framework patches, rendering-cache redesign, or a new test
  framework unless a concrete blocker is demonstrated during implementation.
- Linux support and completing all outstanding native checks for earlier
  slices as a prerequisite to this feature.

## Further Notes

This is MVP roadmap slice 5, following the implemented selection, style, and
text slices. Earlier slices still have native verification gaps that remain
tracked separately.

The pinned GPUI already exposes scroll and pinch events and implements native
pinch delivery on Windows and macOS. This supports implementing the gestures
within the editor; actual hardware behavior still needs native verification.

The shortcut choices, zoom bounds, pan constraints, and resize behavior above
are concrete defaults synthesized for implementation. They were not separately
specified by Kevin. Keep the implementation scoped to these behaviors and
record a demonstrated platform limitation if one requires revisiting them.

## Verification

Implemented with the agreed GPUI editor harness. Automated coverage includes
wheel and pinch input, anchoring and bounds, Fit/100%, pan lifecycle and focus
loss, selection and redo, editing and cropping through a navigated view,
text composition and OS caret bounds, minimum-size toolbar layout, and
resize/DPI behavior. Existing editor tests continue to pass.

Final checks passed:

- `cargo test --workspace`: 61 tests (22 core and 39 editor, including 12
  navigation scenarios).
- `cargo check -p snip-gpui --all-targets`.
- `cargo clippy --workspace --all-targets -- -D warnings`.
- `cargo fmt -- --check` and `git diff --check`.
- `cargo build -p snip-gpui` on Windows.

The Windows executable builds. Native Windows visual, wheel, Space-drag,
pinch, and clipboard checks remain outstanding because the computer-use
helper could not connect to its native pipe (Windows error 2: the system
cannot find the file specified). macOS and real mixed-DPI hardware checks
remain outstanding. Simulated OS events do not count as native verification.

## Comments

The required Standards and Spec reviews were run independently against the
preimplementation HEAD and the working-tree changes. Standards review found
one duplicated padding constant, now shared. Spec review found missing IME
position invalidation, Windows scroll-line normalization, and transformed-text
interaction coverage. All three were addressed; the native wheel-event test
failed before the normalization fix and passed afterward. Both reviewers
rechecked the changes and reported zero remaining findings.

The thermo-nuclear review found no structural blockers. Its follow-up cleanup
consolidated manual view writes, removed the placeholder viewport, grouped
navigation input and controls, centralized view-driven IME invalidation, and
shared Space-release and focus-loss cleanup. The wheel-event fixture retains
its independent OS-setting read so production normalization cannot silently
define both the input and expected behavior of that test.
