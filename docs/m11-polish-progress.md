# M11 polish: first app pass

Done: separate top/rail surfaces and borders; three-colour brand mark; removed Rails
pill and unknown status labels; 380 px search field; uppercase dim headings; active
panel-fill rows, dots and right-side counts; empty agent/file messages; centred
line-box text as the approved stopgap; pane access labels; real duration formatting;
6 px focus/agent glows; chart fill/empty baseline; two-line timeline with real age
labels; named access-switch cards.

The shorter chart heading is `AGENT COMMANDS / MIN · 30 MIN`.
`LookState` adds optional/default-empty `session_counts` (session ID to pane count)
and `timeline_times` (labels parallel to timeline). The app supplies actual pane
counts and ages from owner look_state events' new monotonic `age_ms` field.

Focused look checks passed, unchanged expectations. Measured premultiplied RGBA:

| Point | Dark | Light |
|---|---|---|
| focus (215,577) | (31,41,62,242) | (205,214,231,241) |
| agent (215,222) | (55,30,87,244) | (206,194,230,242) |

Dark agent red minus green is 25 (minimum 20). Light agent blue minus green is 36
(minimum 18), green 194 (maximum 214). Focus keeps the focus colour. Derived agent
glow colours and alpha are documented in design-m11-look.md. All other protected
colour samples and geometry passed. App binaries compile.

Still pending: renderer-measured pill widths, ascent/descent centring and letter
spacing; adjacent shell/driver placement and full search placeholder depend on
those metrics. The command prompt mark is not inserted over terminal cells: that
would overwrite or shift real command/output text. Coordinate its fixture content
with the lead and gate team rather than guessing positions. Existing real command
text keeps the terminal's 16 px sizing; no invented durations or shell names.

Ask clanky-pkg to rerun only the picture step with this app commit, retaining all
fixed sample points. Populate session_counts/timeline_times for the richer fixture;
retain the real 41.2 s fixture duration. Fresh pictures and live visual acceptance
remain outstanding. Settings window integration stays paused until polish is accepted.
