# Crop

Crop keeps the original image and annotations available while copy/save contains
only the chosen region. Corners adjust it; Clear crop restores the full image.

## Sub-features

- Create, replace, adjust, and clear a crop.
- Dimmed excluded area, corner handles, and source-pixel dimensions.
- Escape cancels an active crop gesture; each completed edit undoes in one step.
- Handles/dimensions are absent from exports.

## How to get to it (user POV)

Press `C` or choose Crop, drag a region or a crop corner, and use Clear crop.

## Driving it with snipctl

Preconditions: fixture snip in Fit (`key mod+0`).

- **Create/export.** `key c`, `drag 0 0 300 300`, `shot crop-result`.
  The blue region is kept and green is dimmed. Copy once and inspect: 300×300,
  showing the blue region and any annotations within it, without crop controls.
- **History.** Undo restores the full image; redo restores the crop.
- **Adjustment, when affected.** Drag a visible corner; the opposite corner stays
  fixed. Check the minimum size, replacement, and Clear crop. Each undoes in one step.
  Use a final export to check the resulting region; reuse the shared Copy/Save comparison.
- **Zoom, when affected.** Adjust a corner after zoom/pan and check that the export
  and displayed dimensions describe source pixels.

## Gotchas

- Crop's far corner is exclusive: image point (300,300) keeps 300 pixels.
- Annotations outside the crop reappear when it is cleared or undone.
- Canceling a held drag needs Escape before release; `drag` does not expose that
  timing. Use explicit desktop input or report the native path as untested.
