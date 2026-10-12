# 0078. The title bar follows VS Code's zoom rule

- **Status:** Accepted, implemented; not yet checked by hand in the installed app
- **Recognized:** 2026-10-11, comparing against VS Code at several zoom levels
- **Decided:** 2026-10-11
- **Recorded:** 2026-10-11
- **Confidence:** decided now
- **Evidence:** ADR-0077 (this changes its numbers and its zoomed-out behaviour); ADR-0057
  (the title bar and the snap overlay); VS Code's `titlebarPart.ts` and `titlebarpart.css`
  under its `src/vs/workbench/browser/parts/titlebar/`, read from a clone the maintainer put
  in the repository folder (MIT licence: the idea is taken, the code is not)

## Context

ADR-0077 guessed at the sizes: the controls 230 pixels wide and the bar 34 high, held at
that on the screen, and only the controls and the bar's minimum height. The maintainer measured
VS Code at its zoom extremes and got figures that did not match, and asked what it really does.

VS Code's rule, read from its source, is:

- The window controls take 138 pixels on the screen at every zoom. Zoomed in, the width is
  `138 / zoom` page pixels; zoomed out, the whole title bar is counter-zoomed so it is 138.
- The title bar is 35 page pixels high at least. Zoomed in, it grows with the page (35 times the
  zoom on the screen). Zoomed out, its height is `35 / zoom` page pixels, which is 35 on the
  screen.
- Zoomed out (below 100 percent), the whole bar is counter-zoomed, with CSS `zoom: 1 / zoom`,
  so its menus and search box stay a normal size as well. Zoomed in, only the controls hold their
  size, and the menus and search box grow with the page.
- Dropdown menus opened from a counter-zoomed bar apply the zoom factor again to their own
  box, so they land in the right place.

The maintainer's own measurements of VS Code differ from these numbers because, as far as can be
told from the source, the controls there are drawn by the operating system and measured in
physical pixels at the display's scale, with the page only reserving their space. That is not
something this app can match, and the rule above is what to follow.

## Decision

**ADR-0077's mechanism stays**: the page works out a zoom factor (the pixel ratio divided by the
window's display scale), puts it in `--zoom` on the root, and sizes are written against it. Its
numbers and one behaviour change:

- **The three window controls together are 138 screen pixels wide** (46 each), not 230, and their
  icons are a fixed screen size. The constant `--chrome-controls-w` becomes 138px.
- **The title bar's minimum height is 35**, not 34. `--chrome-bar-min-h` becomes 35px, and
  the rule is VS Code's: 35 page pixels at zoom 1 and above (so it grows on the screen), and
  `35 / zoom` page pixels below zoom 1 (so it stays 35 on the screen).
- **Below zoom 1 the whole title bar is counter-zoomed**: CSS `zoom: calc(1 / var(--zoom))` on the
  bar. Its contents stay at their 100 percent size.
- **Dropdown menus and popovers anchored to the bar apply `zoom: var(--zoom)`** to their own box
  while the bar is counter-zoomed, and must be tested in place at 25, 50, 100 and 200 percent.
  If CSS `zoom` shifts the rectangles the menus read from the bar, they are corrected for it.
- **The snap overlay is re-checked** against the controls at each of those zooms: it reads the
  maximise button's rectangle times the pixel ratio, which a `zoom` on the bar can disturb.

## Rejected alternatives

- **Matching the maintainer's measured figures.** They describe native buttons at a particular
  display scale, so they cannot be reproduced from page code and would vary by machine.
- **Counter-zooming the bar at every zoom.** Above 100 percent the bar growing with the page is the
  VS Code behaviour and what the maintainer asked for ("super tall buttons" at high zoom).
- **Keeping ADR-0077's 230 pixels.** It was a guess, and 138 is the natural width of three
  Windows caption buttons.

## Consequences

CSS `zoom` is applied to a bar the page also positions things against. The menus (ADR-0075) and
the snap overlay are the parts that can be wrong by a zoom factor, and are what the check must
cover. The About line (ADR-0077) stays and helps to read the zoom the shell reports.

## Notes

ADR-0077 is not rewritten: its mechanism holds and this record says what changed.
