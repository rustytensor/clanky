# M12 Settings gate expectations

Worked out from `/srv/devops/build/clanky/mockups/settings.html`, before running
the new panel. Measurements are logical pixels at scale 1; the panel's observed
origin is `(x, y)` so these checks do not assume a window size.

- Outer panel: 780 × 600, corner radius 12, 1 px border.
- Navigation: 200 px wide, sidebar fill, 1 px divider on its right.
- Main title bar and footer: 56 px tall each, with 1 px dividers.
- Setting rows: at least 64 px high, with 1 px dividers. Navigation items: 32 px
  high, radius 6. Label: 14 px, weight 500; description: 13 px, muted.
- Unobstructed panel interior samples (tolerance 3 per RGBA channel): at
  `(x + 4, y + 100)`, dark navigation `(20,18,16,255)`, light navigation
  `(243,240,235,255)`; at `(x + 206, y + 10)`, dark title background
  `(26,24,22,255)`, light title background `(251,250,248,255)`.
  Sample coordinates intentionally avoid labels and borders.
- All displayed labels must avoid raw boolean values `true` and `false` and
  raw opacity `1.00`; for example the opacity control shows `100%`. Inspect
  text runs, not serialized settings or hex colour values.
- Check through the panel's real layout and scene code in both Stone themes.
  Exercise a planted layout or colour fault before counting the check as passed.
