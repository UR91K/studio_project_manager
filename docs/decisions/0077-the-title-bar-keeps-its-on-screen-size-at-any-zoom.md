# 0077. The title bar keeps its on-screen size at any zoom

- **Status:** Accepted, implemented; its sizes and zoomed-out behaviour are changed by ADR-0078
- **Recognized:** 2026-10-11, checking zoom in the installed build (ADR-0073)
- **Decided:** 2026-10-11
- **Recorded:** 2026-10-11
- **Confidence:** decided now
- **Evidence:** ADR-0057 (the title bar is the page's own, with a native snap-layout overlay
  over the maximise button, placed in physical pixels); ADR-0073 (zoom is the webview's own);
  ADR-0032 (the shell's density); the bug `titlebar-breaks-at-extreme-zoom` in
  `docs/bugs.md`, whose first fix (a minimum height and non-shrinking controls in page units)
  did nothing in the shell; `web/shared/os.ts`

## Context

The webview's zoom scales every CSS pixel, and the shell sizes everything in one unit that
follows the root font size, so the title bar, its controls and their icons scale together with
the page. At 25 percent the window controls are a few pixels across, and at 200 percent they
are enormous. The maintainer found both unusable.

The first fix gave the bar a minimum height and stopped the controls shrinking, all in page
units. That cannot work: under uniform zoom, a size in page units is a size that zooms. It
only helped a bar made narrower than its content, which real zoom does not produce, and the
maintainer saw no change in the installed build.

A native title bar does not zoom. The shell draws its own (ADR-0057), so it has to do that
work itself.

## Decision

**The window controls have a fixed width on the screen, whatever the zoom.** Together the
three buttons are 230 screen pixels wide, about 77 each, and their icons are a fixed on-screen
size too. At 25 percent and at 500 percent they are the same width.

**The title bar has a minimum height on the screen, 34 pixels.** Below the zoom at which the
bar's natural height reaches that, it stays at 34. Above it, the bar grows with the zoom as the
rest of the page does, so the buttons, which fill the bar's height, get taller: at 200 percent
they are 230 wide and tall.

Both numbers are named constants, so they can be tuned in one place.

**How.** The page works out the current zoom factor and puts it in a CSS variable on the root
element. Sizes that must not zoom are written as a screen size divided by that variable. The
factor is the webview's device pixel ratio divided by the window's own scale factor (the
operating system's display scaling, which the shell can report), and it is updated whenever
the window is resized or the pixel ratio changes, which is when zoom changes. Outside the
shell there is no zoom of this kind and the factor is 1.

The menu bar, the search box and the rest of the title bar's content keep zooming with the
page. Only the controls' width and icon size, and the bar's minimum height, are held.

**The snap overlay still follows the maximise button.** It already takes the button's real
rectangle times the pixel ratio (ADR-0057), so a button of fixed on-screen size makes that
rectangle steadier, not different.

## Rejected alternatives

- **Page-unit minimums and non-shrinking controls.** Tried first, and it does nothing under
  uniform zoom.
- **Counter-scaling the whole title bar.** The menu bar and search box are content people
  expect to scale with the window, and dropdown menus are anchored to the menu bar.
- **Reading the zoom from the window's outer and inner widths.** It depends on how the
  webview reports them, which is not guaranteed, where the pixel ratio and the scale factor are.
- **Drawing the controls natively.** It is what the native overlay already does for the snap
  flyout (ADR-0057), but the visible buttons are the page's, and moving them is a much larger
  change.

## Consequences

The title bar's height and the controls' sizes now depend on a number computed at run time, so
a wrong factor shows up as wrongly sized controls. It has to be tested in the shell at 25, 100,
200 and 500 percent, and with a display scale other than 100 percent, because the factor
divides one by the other.

At extreme zoom-out the bar is much taller than the page content around it, which is the
intended trade.

## Notes

If the factor cannot be worked out, it is 1 and the bar behaves as before, which is the old
behaviour and not a broken one.
