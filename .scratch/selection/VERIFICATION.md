# Selection verification

Date: 2026-10-10. Implementation is present; native verification is outstanding.

## Automated checks

- `cargo check -p snip-gpui`: passed during implementation.
- `cargo build -p snip-gpui`: passed on Windows.
- `cargo test --workspace`: 30 tests passed: 14 core editor-model tests,
  five core rendering tests, and eleven GPUI editor event/export tests.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.

The editor tests open the real editor component in GPUI's existing test
platform, interact through rendered toolbar controls, keyboard events, and
pointer events, and inspect saved PNG pixels. The OS file picker is simulated;
the editor's private state and methods are not used as test interfaces. These
tests use GPUI's debug selectors and run in the default debug test profile.

Editor scenarios cover selecting, moving and deleting with one-step undo/redo;
all rectangle corners crossing their opposite corner; both arrow endpoints;
invalid final geometry; Shift constraints; stationary-pointer Shift changes
with final geometry and one-step history; text double-click/re-edit; pencil
marks; high-DPI and fitted images; handle grab offsets; cancelled and unchanged
edits; preserving redo after an out-and-back resize; release outside the image;
tool changes and export during a live move; crop preservation; selection-free
export; and Escape precedence.

## Standards review

No material findings. The review confirmed use of the existing core renderer
and history, transient editor selection, focused external-behavior tests, and
updated comments and usage documentation. No speculative framework was added.

## Spec review

No material code or scope findings. The review requested stationary-pointer
modifier-event coverage, which was added. It confirmed that native verification
must remain explicitly outstanding. Final exported geometry and history are
tested; immediate live visual snapping is not proved by export tests because
mouse-up recomputes geometry.

## GPT-6.1 Sol review follow-up

The fresh review found one P2 defect: an unchanged Shift-resize of a vertical
arrow could discard redo. Trigonometric snapping changed the endpoint's x
coordinate from `40.0` to `39.999996`, recording an invisible history edit.

The defect was reproduced first through GPUI editor pointer/keyboard events:
draw an arrow and rectangle, undo the rectangle, Shift-drag the arrow endpoint
away and back, then redo. Before the fix, the exported PNG lacked the rectangle.
A focused public core test independently reproduced the coordinate drift.

Snapping now returns already constrained endpoints unchanged and uses explicit
octant components so newly snapped cardinal directions keep the fixed axis
exact. History equality is unchanged. Both regression tests pass: the editor
test proves redo restoration and no hidden undo step, and the core test checks
exact preservation in all eight directions with integer and fractional anchors.
Existing snapping tests now require exact cardinal results.

The code finding is addressed. Native platform checks remain outstanding.

## Native verification still required

Windows computer-use discovery was attempted twice. Both attempts reported:

> Computer Use native pipe is unavailable: failed to connect native pipe: The system cannot find the file specified. (os error 2)

No native editor UI interactions were performed through that runtime. macOS
was not available in this environment. Simulated 1×/2× scale and reduced-fit
tests are not proof of actual monitor behavior.

Before marking the slice fully verified on both platforms, exercise the real
shortcut → snip → editor workflow, inspect outlines, handles, cursors and
continuous feedback, toggle Shift with a stationary pointer, verify actual
double-click/keyboard/drag delivery and release outside the image, and save
and copy while selected or mid-drag. Paste the copied result into an
image-capable application and confirm that edited pixels agree with save and
that selection controls are absent. Include an actual high-DPI display and a
screenshot that fits below its native resolution.
