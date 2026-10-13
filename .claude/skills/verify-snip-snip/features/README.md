# snip-snip verification map

Start with [the verification skill](../SKILL.md) for launch, coordinates, the
routine check, evidence, and cleanup. These recipes describe targeted checks;
choose those affected by the change.

## Features

- [Snip](snip.md): hotkey, frozen overlay, selection, cancel.
- [App shell](app-shell.md): tray/menu-bar, Settings, capture shortcut, Quit.
- [Annotate](annotate.md): tools, text, selection, handles, styles.
- [Navigation](navigation.md): zoom, pan, Fit/100%, resize, editing at zoom.
- [Crop](crop.md): crop creation, adjustment, clearing, exports.
- [Undo and redo](undo-redo.md): history across annotations and crop.
- [Export](export.md): Copy, Copy & Close, Save, cancellation, failure handling.

## Native platform coverage recorded on 2026-10-12

Windows MVP feature coverage was recorded; native pinch and delivery of the
Copy & Close shortcut remain unproven (`Ctrl+Shift+C` was globally reserved).
Its toolbar completion flow was verified.

macOS passed physical shortcut delivery, Settings recording/application,
restart persistence, Canadian/Dvorak input-source switching, capture, and
clipboard readback matching the native fixture region. Native annotation,
text/IME, navigation, crop, Save, Copy & Close, menu capture/Quit, and
shortcut conflict/error recovery remain partial or unverified. Hardware pinch
needs a touchpad. This historical coverage does not replace checking a change.

macOS evidence was recorded under `target/verify/1791843397808/` and
`target/verify/1791843849110/`; build artifacts may have been cleaned since.
