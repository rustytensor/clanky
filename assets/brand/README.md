# Clanky icon and logo

The app icon, symbolic icon, logo lockups and favicon for Clanky. The drawing is Clanky the robot standing in a
terminal window, head tilted and breaking out through the top of the frame.

## Files

| File | Use |
|---|---|
| `hicolor/scalable/apps/clanky.svg` | Master app icon (full drawing, 512 unit canvas). Install as the scalable icon. |
| `hicolor/<N>x<N>/apps/clanky.png` | App icon PNGs for 16, 22, 24, 32, 48, 64, 128, 256 and 512 px. Sizes up to 32 px are rendered from the small-size drawing, larger sizes from the master. |
| `source/clanky-small.svg` | Small-size drawing: heavier window frame, bigger eyes, no title-bar dots, brow flaps or antenna arcs. Source for the 16 to 32 px PNGs and the favicon. Do not use it above 32 px. |
| `hicolor/symbolic/apps/clanky-symbolic.svg` | One-colour icon for panels and trays, drawn on a 16 px grid in `currentColor`: the head above an open-topped window outline. |
| `logo/clanky-logo-horizontal-on-dark.svg`, `...-on-light.svg` | Horizontal lockup (icon left, word right) for dark and for light backgrounds. Transparent background; the word is in outlines. |
| `logo/clanky-logo-stacked-on-dark.svg`, `...-on-light.svg` | Stacked lockup (icon above word), same rules. |
| `logo/clanky-logo-horizontal-on-dark@2x.png` and the three other `@2x.png` files | The four coloured lockups as transparent PNGs at twice their nominal size (horizontal 240 px tall, stacked 480 px wide). |
| `logo/clanky-logo-horizontal-mono.svg`, `logo/clanky-logo-stacked-mono.svg` | One-colour lockups: everything is `currentColor`, so the colour comes from the surrounding CSS or the print job. They use the symbolic head-and-window mark. For print, README badges and any place the full-colour icon will not reproduce. |
| `favicon/favicon-16x16.png`, `favicon/favicon-32x32.png`, `favicon/favicon.ico` | Favicon PNGs and an .ico holding both sizes, from the small-size drawing. |

All SVGs are plain SVG with a `viewBox`, one `title`, no editor metadata and no scripts, and are under 20 KB each.
The wordmark is Inter SemiBold (SIL Open Font License, shipped in `assets/fonts`) converted to outlines, so it
renders the same without the font installed.

## Colours

| Name | Hex | Where |
|---|---|---|
| Graphite screen | `#141414` | Window screen, outline around the robot, wordmark on light backgrounds |
| Graphite frame | `#242424` | Window frame and title bar |
| Frame edge | `#383838` | Thin edge of the frame (full drawing); `#7d776e` on the small-size drawing |
| Stone | `#d9d2c7` | Head housings, torso, highlights |
| Stone dark | `#b0a79b` | Jaw, neck, shoulders, arms, brow flaps |
| Amber | `#e3a363` | Eyes, antenna light, chest display and charge meter, prompt cursor |
| Wordmark on dark | `#ebe6de` | The word "Clanky" on dark backgrounds |
| White | `#ffffff` | The light background the on-light files are made for |

## Clear space and minimum size

- Keep clear space on all four sides of a lockup equal to half the height of the capital C in the word. In the
  horizontal lockup that is about 24 % of the lockup's height; in the 2x horizontal PNG, 58 px.
- Nothing else (text, edges of the page, other logos) goes inside that space.
- Do not shrink the horizontal lockup below 28 px tall or the stacked lockup below 64 px wide; below that, use the
  icon alone, or the symbolic icon where the surface wants one colour.
- Use the coloured lockups on dark or white. On any other background use a one-colour lockup.
- Do not recolour the robot, stretch the lockup, or redraw the head.
