# MVP roadmap

The MVP workflow: press a shortcut, drag a rectangular selection, annotate or
crop in the editor, then save or copy. It targets Windows and macOS; Linux
comes later. Each slice gets its own `PRD.md` under `.scratch/` when it is
picked up.

1. **Editor core model (complete):** arrow and rectangle shapes, hit testing,
   bounds, moving, and undo/redo.
2. **Selection (implemented):** a Select tool, a selection outline, dragging to move, deleting,
   handles for arrow endpoints and rectangle corners, double-click to re-edit
   text. Shift constrains arrows to 45° steps and rectangles to squares.
   Automated checks pass; native Windows/macOS checks remain outstanding.
3. **Style (implemented):** five colors and three stroke widths, shared across
   annotation tools. Changing a style also restyles the selected annotation,
   with undo/redo. Text uses color only. Automated checks pass; native
   Windows/macOS checks remain outstanding.
4. **Text (implemented):** multi-line text, a movable caret, and OS text
   input through GPUI's input handler, so IME and dead keys work. Automated
   checks pass; Windows was verified natively except a real IME and dead keys
   (none installed). macOS checks remain outstanding.
5. **Zoom and pan (implemented):** Ctrl/Cmd+scroll or pinch to zoom, Space+drag
   to pan, and 100% and fit shortcuts and toolbar controls. Cursor-anchored
   zoom, bounded panning, and DPI-aware views preserve editing and exports.
   Automated checks pass; native Windows/macOS checks remain outstanding.
6. **Crop polish:** handles to adjust the crop, a way to clear it, and the
   pixel size shown while dragging.
7. **Save flow:** a default folder and a timestamped filename, plus copy and
   close in one action.
8. **App shell:** a tray/menu-bar icon, launch at login, and a configurable
   hotkey.
