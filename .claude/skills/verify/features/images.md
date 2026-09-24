# Images

Markdown images render inline at full resolution. The path is resolved relative to the Markdown file. markview uses the kitty graphics protocol and falls back to Sixel, iTerm2, or colored half blocks. A missing or unreadable image shows `[image not loaded: <alt or path>]` in muted italics. Images crop cleanly as they scroll past the top or bottom edge.

## Sub-features

- `image-inline` draws PNG, JPEG, WebP, and GIF inline, scaled to fit the width.
- `image-relative` resolves paths against the Markdown file's directory, not the shell's directory.
- `image-missing` shows `[image not loaded: <alt>]`, or the path when alt text is empty.
- `image-crop` shows only the visible rows of a partly scrolled image.

## How to get to it (user POV)

- Open `sample.md` and scroll to `Offsite`, which has `![Fjord from the trail](assets/photo.jpg)`.
- Open a document whose image link is broken.

## Driving it with mv-verify

Preconditions:

- `$V launch image-1 sample.md` is ready.

- **Inline.** Run `$V key image-1 g SHIFT` and `$V wait image-1 'sample.md  100%'`. Run `$V shot image-1 bottom`. Under the `Offsite` paragraph, the PNG shows the fjord photo in full color, not in half-block pixels. `sample.md` has only a JPEG. For PNG, WebP and GIF, convert `assets/photo.jpg` with `magick` into `target/verify/<run>/` and reference them from a fixture there.
- **Crop at the bottom edge.** From 100%, press `k` about 6 times, until the status bar drops below `97%`. `$V shot image-1 cropped` shows the bottom of the photo cut cleanly just above the status bar, with no smear onto it.
- **Crop at the top edge.** `sample.md` can't show this, because the photo stays on screen at 100%. Write a fixture in `target/verify/<run>/` with the image first and about 60 lines of text after it. Press `j` until the photo's top rows leave the screen. The screenshot shows only its lower part, cut cleanly at the first row.
- **Relative path.** `launch` always starts markview in the file's directory, so a normal launch can't tell a path resolved from the file apart from one resolved from the working directory. Write `target/verify/image-2/rel/doc.md` containing `![Fjord](pics/fjord.jpg)` and copy `assets/photo.jpg` to `target/verify/image-2/rel/pics/fjord.jpg`. The repo root has no `pics/`. Launch `$V launch image-2 sample.md`, then open the fixture from the repo root in an overlay with `$V kitty image-2 launch --type=overlay --cwd=$PWD target/debug/markview target/verify/image-2/rel/doc.md`, then `$V wait image-2 'rel/doc.md  [0-9]+%'` and `$V settle image-2`. `$V shot image-2 rel` shows the photo.
- **Missing image.** Write `target/verify/image-3/missing.md` containing `# Fixture`, `![Missing chart](does-not-exist.png)`, `![](gone.png)`, and `![Corrupt](corrupt.png)` as separate paragraphs, and write plain text into `target/verify/image-3/corrupt.png`. Then `$V launch image-3 target/verify/image-3/missing.md`. `$V text image-3 screen` contains `[image not loaded: Missing chart]`, `[image not loaded: gone.png]`, and `[image not loaded: Corrupt]`.
- **Proof.** `bottom.png` and `cropped.png` from `image-1`, the top-edge crop screenshot, `rel.png` from `image-2`, and `screen.txt` from `image-3`.

## Gotchas

- `get-text` shows nothing useful where an image is drawn. Only the screenshot proves an image rendered.
- Images draw at their native pixel size when that fits. Scaling to the window width needs an image wider than the window, about 1900px at the default output.
- Path resolution, sizing, the fallback text, and kitty placement and cropping are markview's. Sixel, iTerm2, and half-block encoding are `ratatui-image`'s.
- The Sixel, iTerm2, and half-block fallbacks can't be reached from kitty. Report them as not covered instead of inferring them from the kitty path.
