# M8 compose results

`v0.4.4-compose.2`: `scripts/gate-m8-compose.sh` passed, exit 0.
The planted send fault failed at the exact 33-byte PTY comparison, exit 1.
Fault and clean ran once each. No expected result changed.

Verified exact en_US flags/ranges and suggested corrections, personal-word reload,
per-terminal draft/selection retention, undo/redo, failed-send retention, one input
call, exact 33-byte bracketed paste and optional 34th CR byte, control rejection,
missing-dictionary fallback and underline pixels. Real PTY receivers captured
trailing input too. Personal lists and receiver output were generated fixtures;
no real personal list or prompt was used. Rendering used software Vulkan.

Clean regressions passed once each: M7 panel (including M1–M4), M7 editor core,
M6 masking and terminal cwd. M5 is not on this branch. Desktop IME, pointer and
suggestion interactions, window-loss handling and application-specific paste
behavior remain manual acceptance, not claims from the offscreen gate.

Dependency: spellbook 0.4.2, MPL-2.0, 691,342 compressed bytes; the approved licence
exception is in AGENTS.md. It reuses hashbrown 0.17.1 (MIT OR Apache-2.0); no other
package was added. Default hasher features are disabled in favor of std RandomState.

Decisions: drafts stay memory-only and are retained after successful send as well
as Escape; new personal lists are private files created only on Add. The overlay
names its captured terminal. Connection loss does not automatically discard
retained drafts; send never switches to another terminal. No server API or
serialized terminal-snapshot changes were made.
