# MVP roadmap

The MVP workflow: press a shortcut, drag a rectangular selection, annotate or
crop in the editor, then save or copy. It targets Windows and macOS; Linux
comes later. Each slice gets its own `PRD.md` under `.scratch/` when it is
picked up.

1. **Editor core model:** arrow and rectangle shapes, hit testing, bounds,
   moving, and undo/redo. See `editor-core-model/PRD.md`.
2. **Selection:** a Select tool, a selection outline, dragging to move, deleting,
   handles for arrow endpoints and rectangle corners, double-click to re-edit
   text. Shift constrains arrows to 45° steps and rectangles to squares.
3. **Style:** a palette of about five colors and three stroke widths, shared by
   all tools. Changing it also restyles the selected annotation.
4. **Text:** multi-line text, a visible caret, and OS text input through GPUI's
   input handler, so IME and dead keys work.
5. **Zoom and pan:** Ctrl/Cmd+scroll or pinch to zoom, Space+drag to pan, and
   100% and fit shortcuts.
6. **Crop polish:** handles to adjust the crop, a way to clear it, and the
   pixel size shown while dragging.
7. **Save flow:** a default folder and a timestamped filename, plus copy and
   close in one action.
8. **App shell:** a tray/menu-bar icon, launch at login, and a configurable
   hotkey.
