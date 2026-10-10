# Crop

`C` then a drag on the image marks a crop rectangle; everything outside is dimmed in the editor and excluded from copy and save. A new drag replaces the crop.

## Sub-features

- `crop-set` dragging sets the crop; the outside is dimmed and the kept area outlined in white.
- `crop-export` copy and save contain only the cropped pixels.
- `crop-cancel` `Esc` mid-drag restores the previous crop.

## How to get to it (user POV)

- Key `C` or the `Crop` toolbar button, then drag on the image.

## Driving it with snipctl

Preconditions:

- Baseline snip open (600x300 two-color image).

- **Set crop.** `snipctl key c`, `snipctl drag 0 0 300 300`, `snipctl shot crop-set`. The green half is dimmed; the blue half is outlined.
- **Export is cropped.** `snipctl key mod+c`, `snipctl clipboard crop`, `snipctl inspect crop`. Expect `300x300 blue=90000 green=0`.
- **Undo.** `snipctl key mod+z`, then copy and inspect again: back to `600x300`.

## Gotchas

- The crop's far corner is exclusive: a drag to image point (300,300) keeps 300 px; a drag to (299,299) keeps 299.
- Annotations outside the crop stay in the document and reappear if the crop is undone.
- `crop-cancel` needs `Esc` while the button is held, which `snipctl drag` cannot do; report it as skipped.
