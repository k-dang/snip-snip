# MVP roadmap

The MVP workflow: press a shortcut, drag a rectangular selection, annotate or
crop in the editor, then save or copy. It targets Windows and macOS; Linux
comes later.

1. **Editor core model (complete):** arrow and rectangle shapes, hit testing,
   bounds, moving, and undo/redo.
2. **Selection (implemented):** a Select tool, a selection outline, dragging to move, deleting,
   handles for arrow endpoints and rectangle corners, double-click to re-edit
   text. Shift constrains arrows to 45° steps and rectangles to squares.
   Automated checks pass; native Windows selection, handles, constraints,
   deletion, text re-editing, and undo/redo pass. macOS checks remain outstanding.
3. **Style (implemented):** five colors and three stroke widths, shared across
   annotation tools. Changing a style also restyles the selected annotation,
   with undo/redo. Text uses color only. Automated checks and native Windows
   colors, widths, shared styles, text controls, restyling, and undo/redo pass.
   macOS checks remain outstanding.
4. **Text (implemented):** multi-line text, a movable caret, and OS text
   input through GPUI's input handler. Automated checks pass; Windows was
   verified natively. macOS checks remain outstanding.
5. **Zoom and pan (implemented):** Ctrl/Cmd+scroll or pinch to zoom, Space+drag
   to pan, and 100% and fit shortcuts and toolbar controls. Cursor-anchored
   zoom, bounded panning, and DPI-aware views preserve editing and exports.
   Automated checks and native Windows wheel zoom, Space-pan, bounds, Fit/100%,
   resizing, zoomed editing, and export preservation pass. Native pinch needs
   touchpad hardware; macOS checks remain outstanding.
6. **Crop polish (implemented):** four corner handles to adjust the crop,
   Clear crop, and image-pixel dimensions shown while dragging. Creation,
   adjustment, replacement, and clearing each undo in one step. Automated
   checks pass; Windows capture, crop editing, save, and clipboard readback
   were verified natively. Windows corner cursors and clipboard paste into
   Paint also pass. macOS checks remain outstanding.
7. **Save flow (implemented):** a platform Pictures folder with a home
   fallback, readable timestamped filenames and collision suffixes, plus
   Copy & Close after clipboard success. Automated checks pass; Windows
   picker defaults, cancellation, save, copy, native toolbar completion,
   clipboard readback after closure, paste into Paint, and another capture
   were verified.
   Ctrl+Shift+C is reserved globally on the verification machine, so native
   shortcut delivery remains outstanding, as do macOS checks.
8. **App shell (implemented):** a native tray/menu-bar icon with New snip,
   Settings, and Quit, plus a persistent configurable hotkey and visible
   failure recovery. Automated checks pass; Windows shell controls, shortcut
   replacement/restart, menu dismissal before capture, and Quit were verified
   natively. Combined Save flow toolbar Copy & Close, clipboard readback, and
   another capture also pass on Windows. macOS checks remain outstanding.
