Status: implemented

# Text

Roadmap slice 4: multi-line text, a visible caret, and OS text input through
GPUI's input handler, so IME and dead keys work.

## Problem Statement

Text labels in the editor are single-line and append-only. Typing always adds
to the end of the label, Backspace only removes the last character, and the
caret is pinned to the end. A user who mistypes a word in the middle of a
label must delete back to it and retype everything after. Enter ends the label,
so a two-line callout takes two separate labels that the user must line up by
hand.

Keystrokes are read straight from key-down events, not through the OS text
input system. Users who type with an IME (Japanese, Chinese, Korean), or with
dead keys on international layouts (´ then e for é), cannot enter their
language: composition never starts, or produces stray characters.

## Solution

A text label is a multi-line block. While editing, Enter starts a new line, and
Escape or clicking elsewhere ends the session. The label has a real caret that
the user moves with the arrow keys, Home/End, or a click inside the label.
Typing, Backspace, and Delete act at the caret.

All typed text comes from the OS through GPUI's input handler. IME composition
shows inline, underlined at the caret, with the candidate window anchored next
to it. Dead keys combine as they do in any native text field. A whole editing
session is still one undo step, as it is today.

## User Stories

1. As a user annotating a screenshot, I want Enter to start a new line in my label, so that I can write a multi-line callout as one annotation.
2. As a user, I want Escape to end text editing, so that I can finish a label without reaching for the mouse.
3. As a user, I want clicking outside the label to end editing, so that I can move on to the next annotation in one motion.
4. As a user, I want switching tools to end editing, so that a label is never left half-edited.
5. As a user, I want copy and save to include the label I am typing, so that I never lose the last edit by exporting mid-session.
6. As a user, I want a visible caret at my insertion point, so that I know where typed text will go.
7. As a user, I want the caret to sit on an empty new line right after I press Enter, so that I can see that the line exists before I type on it.
8. As a user, I want Left and Right to move the caret one character, so that I can fix a typo without deleting what follows.
9. As a user, I want Left and Right to step over a whole emoji or accented character, so that the caret never lands inside one.
10. As a user, I want Up and Down to move the caret between lines, keeping its horizontal position as closely as the line allows, so that moving between lines feels like any text field.
11. As a user, I want Home and End to jump to the start and end of the current line, so that I can get to the line edges quickly.
12. As a user, I want Backspace to delete the character before the caret, so that I can correct mistakes anywhere in the label.
13. As a user, I want Delete to remove the character after the caret, so that I can delete forward like in other editors.
14. As a user, I want Backspace at the start of a line to join it with the line above, so that I can undo an accidental Enter.
15. As a user, I want typing in the middle of a line to insert text, so that I can add words I forgot.
16. As a user, I want clicking inside the label I am editing to move the caret there, so that I can jump straight to the spot I want to change.
17. As a user, I want double-clicking an existing label with the Select tool to re-edit it with the caret at the point I clicked, so that I can edit right where I am looking.
18. As a user, I want clicking an existing label with the Text tool to re-edit it with the caret at the click point, so that I can edit labels without switching tools.
19. As a Japanese, Chinese, or Korean user, I want IME composition to work in labels, so that I can annotate in my language.
20. As an IME user, I want text I am still composing to appear inline and underlined at the caret, so that I can see what I am composing in place.
21. As an IME user, I want the candidate window to appear next to the caret, so that I don't have to look across the screen to pick a candidate.
22. As an IME user, I want committing a candidate to replace the composition with the chosen text, so that only the final text stays in the label.
23. As an IME user, I want text still being composed to be kept, not discarded, when I click away or press a shortcut mid-composition, so that I don't lose input.
24. As a user on an international keyboard layout, I want dead keys to combine with the next character, so that I can type accented letters.
25. As a user, I want a whole editing session, including newlines and caret moves, to undo in one step, so that undo matches how I think about a label.
26. As a user, I want re-editing an existing label to undo back to its previous content in one step, so that I can safely experiment with changes.
27. As a user, I want a label left empty or containing only whitespace to vanish when I finish editing, so that invisible annotations don't clutter the document.
28. As a user, I want the selection outline and hit area of a multi-line label to cover all its lines, so that I can select and move it by clicking any line.
29. As a user, I want a multi-line label to render identically in the editor and in the saved or copied image, so that what I see is what I export.
30. As a user, I want the label's lines to keep the label's color, including after restyling, so that multi-line labels behave like single-line ones.
31. As a user, I want Ctrl/Cmd shortcuts (undo, copy, save) to keep working while editing text, so that typing mode never traps my usual commands.
32. As a user, I want tool shortcut letters (V, P, A, R, T, C) to type characters while I am editing a label, and not switch tools, so that I can type any word.
33. As a user on a high-DPI display, I want the caret and IME candidate window to line up with the text, so that editing looks right on Retina and scaled Windows displays.
34. As a user on a fitted (scaled-down) large screenshot, I want caret placement by click to land where I clicked, so that editing works at any zoom-to-fit scale.

## Implementation Decisions

- **Document model.** `Text` is a multi-line block: `content` may contain `\n`. Lines are never wrapped automatically. The label is as wide as its widest line and as tall as its line count times the existing line height. The type's doc comment changes from "single-line label" to match. No new fields are added. Caret and composition state belong to the editing session, not the document.
- **Text layout queries in snip-core.** The renderer already owns cosmic-text layout. It gains a few queries built on the same layout that painting uses, so the caret, hit testing, and pixels cannot disagree:
  - the label's size, which now counts an empty trailing line, so the caret on a new last line has room and the bounds include it;
  - the caret rectangle (top-left and height, in image pixels) for a byte index in `content`;
  - the byte index nearest to a point in image pixels, for click-to-place;
  - the byte index one visual line up or down from an index, keeping the caret's x position, for Up/Down;
  - the byte range of the line containing an index, for Home/End, and the range of any `content` substring, so the editor can underline composing text and report positions for the IME.
  Character stepping for Left/Right/Backspace/Delete uses grapheme cluster boundaries via a Unicode segmentation crate, so the caret can never sit inside a cluster. These queries take a `Text` and an index. Indices are UTF-8 byte offsets into `content`, always on grapheme boundaries.
- **Editing session in the editor.** The editing state grows from "index of the label being edited" to a small session value: the annotation index, the caret byte offset, and an optional marked (composing) byte range. All of it is cleared when the session ends.
- **OS text input.** The editor implements GPUI's `EntityInputHandler`. While a label is being edited, the canvas registers it with `window.handle_input` each frame; at all other times no input handler is registered. As a result, letters reach the input handler only while editing, and otherwise reach the tool-shortcut key handler.
  - Range parameters in the trait are UTF-16 offsets. The editor converts at this boundary, and everything inside uses byte offsets.
  - `replace_text_in_range` replaces the given range, or the marked range, or else inserts at the caret. It clears the marked range and puts the caret after the inserted text.
  - `replace_and_mark_text_in_range` does the same and marks the inserted text as composing. Composing text is stored in `content`, so the preview renders it with no special path. The overlay underlines the marked range.
  - `selected_text_range` reports an empty range at the caret. `marked_text_range` and `unmark_text` work on the marked range. `text_for_range` returns substrings of `content`.
  - `bounds_for_range` maps the caret or range rectangle from image pixels to window coordinates through the current viewport, so the IME candidate window sits beside the caret on any DPI or fit scale.
  - `character_index_for_point` uses the point-to-index query.
- **Key handling.** Key-down still handles Ctrl/Cmd shortcuts first, and those work during editing. During editing, it also handles the keys that edit the session without inserting characters: Left/Right/Up/Down, Home/End, Backspace, Delete, and Escape. Enter inserts `\n` at the caret, and it must produce exactly one newline whether it arrives as a key or through the input handler. Printable characters are no longer read from `key_char` in the key handler. The input handler alone inserts them.
- **Ending a session.** Escape, a click outside the edited label, a tool change, undo/redo, copy, and save all end the session through the one existing finish path. A composition in progress is committed as-is, by unmarking it. Labels that are empty or whitespace-only are removed. Otherwise the session commits one history entry, as it does today.
- **Clicking during a session.** A click inside the label being edited moves the caret there without ending the session or committing history. Re-entering a label, by double-clicking with Select or clicking with the Text tool, starts a session with the caret at the click point, not at the end of the content.
- **Caret painting.** The caret stays a steady line drawn by the canvas overlay, now at the session's caret rectangle and not at the end of the content. The overlay also draws an underline under the marked range while composing. The preview texture never contains the caret or the underline.

## Testing Decisions

- Good tests here drive the product the way a user or the OS does, and check what the user gets: the saved PNG, which is compared with snip-core's own rendering of the expected annotations. They don't inspect private editor state such as the caret offset. The caret position is observed through its effects, for example where typed text lands, and through what the OS can see (`bounds_for_range`).
- **Seam 1: editor harness** (the existing GPUI `VisualTestContext` tests in the editor test module). Events go in through `simulate_keystrokes`, `simulate_input`, and simulated mouse clicks. Results come out as the saved PNG via the simulated file picker, compared against `Renderer::export` of an expected document. IME is driven by calling the editor entity's `EntityInputHandler` methods in the order an OS IME calls them. That trait is the interface the OS uses, so these calls are the closest automated stand-in for a real IME. Cases:
  - typing, Enter, and more typing saves one multi-line label;
  - arrow keys, Home/End, and Up/Down followed by typing put text at the expected place, including across multi-byte graphemes;
  - Backspace at a line start joins lines, and Delete removes forward;
  - a click inside the edited label moves the caret without ending the session, and re-editing by double-click or by the Text tool starts at the click point;
  - composition and commit save only the committed text, and clicking away mid-composition keeps the composing text;
  - `bounds_for_range` at the caret falls inside the canvas next to the label on high-DPI and fitted layouts;
  - a session that adds lines and moves the caret undoes in one step;
  - a whitespace-only label is dropped with no history entry;
  - tool letters type text during editing and switch tools outside it.
  - Prior art: the double-click re-edit test, the text-color tests, and the high-DPI tests in the same module.
- **Seam 2: snip-core model tests** (the existing `editor_model` tests), only for the new layout queries, where the arithmetic is easiest to pin down: multi-line size and an empty trailing line, round trips between caret rectangle and point-to-index across multi-byte content, and line ranges for Up/Down/Home/End. Prior art: the text hit-region test and the test that a text session undoes in one step.
- Native verification on Windows and macOS uses the `verify-snip-snip` skill. It types a multi-line label, edits mid-line, and checks IME composition with the OS IME (Microsoft IME on Windows, the built-in Japanese input on macOS) and a dead key on an international layout. Screenshots serve as proof. Automated tests cannot cover the real IME, so this step is required before the slice is complete.

## Out of Scope

- Selecting a range of text inside a label (Shift+arrows, drag-select, select all) and cutting, copying, or pasting text. Ctrl/Cmd+C keeps copying the image.
- Word-wise movement and deletion (Ctrl/Alt+arrows, Ctrl/Alt+Backspace).
- Caret blinking.
- Automatic wrapping, or a fixed width for text boxes.
- Text size, font, alignment, background, or outline options.
- Undo steps inside a session. Undo while editing ends the session and undoes it as a whole.

## Further Notes

- GPUI's test window passes a keystroke's `key_char` to the registered input handler when the key handler doesn't consume it, so `simulate_input` keeps working as a way to type in tests once typing moves to the input handler.
- Two earlier slices (Selection, Style) still need their native Windows/macOS checks. Run them alongside this slice's native verification, because double-click re-edit and text restyling interact with the new session behavior.
