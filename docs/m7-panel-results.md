# M7 editor panel results

`v0.4.4-panel.3`: `scripts/gate-m7-panel.sh` passed, exit 0, including M4, M3,
M2 and M1 once each. The planted `editor-save` fault failed at item 1's exact
saved-byte comparison, exit 1. Fault and clean gates each ran once.

Verified: real-shell `cd` and terminal-path opening, Unicode cursor/selection,
typing and undo/redo, mouse selection, find, CRLF insertion, per-hunk review,
accept-all/reject-all, stale-file refusal, exact save bytes, review/selection
background pixels, and visible-line rendering near EOF of a 16,777,222-byte file.
Offscreen rendering used the software Vulkan driver. No expected result changed.
The cwd operation was separately committed and checked in `v0.4.4-panel.2`.

The GUI uses the same controller. Editor splits are window-local, independent
of server terminal layouts. Proposal JSON import is explicit; no new proposal
socket endpoint was added. Only the existing local clanky-edit dependency was
added to the app, with its MIT OR Apache-2.0 licence.

Not manually verified: real desktop keyboard/IME, dragging, zoom, palette entry,
window resize and dirty-close behaviour. Remote terminal file clicks are refused;
remote file transport and editor persistence are not implemented. The person's
terminal display is not passed through the masking engine by this change.
