# M11 polish: first app pass

Done: separate top/rail surfaces and borders; three-colour brand mark; removed Rails
pill and unknown status labels; 380 px search field; uppercase dim headings; active
panel-fill rows, dots and right-side counts; empty agent/file messages; centred
font-metric-centred text; measured pill widths with 9 px side padding; pane access labels; real duration formatting;
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

Renderer f3b7a23 is integrated: measured advances, ascent/descent centring, 0.1 em
heading tracking, measured ellipsis and adjacent title/driver placement. Live windows
measure using their configured font families; offscreen scenes use bundled fonts.
Round 2 adds optional hello client_name/model metadata and owner-only display
labels (`LookState.agent_labels`, keyed by connection ID). Without a name the label
is `agent <pid>`. Gate fixtures may populate this map directly.

Actions now rest hidden, fading in over 120 ms on hover or keyboard card focus.
F6 cycles visible cards; Left/Right/Tab selects an action; Enter activates it;
Escape exits card focus. The agent-stop shortcut stays first. Live mouse/keyboard
acceptance remains manual.

The separate 16 px accent prompt mark measures 9.633 px wide. It fits in the 11 px
between the pane's inner border and grid origin; neither cells nor the grid move.
Both themes' protected colour samples, including (230,451), pass with the mark.
Socket identity/lifecycle checks and app compilation pass. Existing real command
text keeps the terminal's 16 px sizing; no invented durations or shell names.

Ask clanky-pkg to rerun only the picture step with this app commit, retaining all
fixed sample points. Populate session_counts/timeline_times for the richer fixture;
retain the real 41.2 s fixture duration. Fresh pictures and live visual acceptance
remain outstanding. Settings window integration stays paused until polish is accepted.
