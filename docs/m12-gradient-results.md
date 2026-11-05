# Theme window gradients

Fixed the missing backdrop in `calm::presentation`. It previously cleared to the
sidebar token; AI Bedazzled's reported (128,20) pixel was (22,13,41), not its gradient.

Keep this scene-initialization call before the inset and controls:
`theme_scene::window_background(theme, layout.window_px, state.opacity)`.
It handles AI Bedazzled, Unicorn Tears and Unicorn Vomit from their existing stops.
Themes without gradients retain the existing sidebar clear colour.

The renderer's `Fill::VerticalStops` supports 2–8 evenly spaced stops. Segments
retain the original rectangle for interpolation/edge coverage and clip their own
bands, avoiding antialiased seams at fractional stop positions. Settings' existing
scene-fade match gained the corresponding stop-alpha case; no controls/layout changed.

Check: the existing `m12-gate` branch's `clanky-calm-gate` observations, compared with
`gate-m12-calm.py`'s `pixels()` function for AI Bedazzled only, using software Vulkan.
The harness was copied temporarily; no gate code or expectations were changed.

| Pixel | Expected, tolerance 3 | Observed |
|---|---|---|
| (128,20) | (122,59,237,255) | (122,59,237,255) |
| (128,740) | (39,98,235,255) | (39,98,235,255) |

PASS. No full gate, unrelated checks or separate unicorn pixel checks were run.
