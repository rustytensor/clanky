# Window polish results

`v0.4.4-polish.3`: `scripts/gate-polish-window.sh` passed, exit 0.
The planted wrapped-search fault failed at the expected two-match comparison,
exit 1. Fault and clean each ran once. No expected result changed.

Verified real wrapped history/next/previous/highlight cells, hard-line separation,
URL argv and underline pixels, inert plain URL clicks, server-side folder/home
inheritance, OSC 52 complete/split writes, exact 1 MiB acceptance and oversized
refusal, default read denial, opt-in read bytes, bounded queues, single-use replies
and rejection of replies after a command changes. Browser/clipboard values were
controlled fixtures, never the person's real desktop data.

The clean gate ran M8 once, including M7 panel/core, M6 masking, cwd and M1–M4 once
each. Renderer variables were unset: the separately committed software defaults
worked throughout. M5 is not on this branch. No new dependencies or schema changes.

Decisions/limits: URLs must be complete in one visible physical row; search does
join soft-wrapped rows. Clipboard writes default on, at most 1 MiB; reads default
off and are capped at 64 KiB when enabled. Wire clipboard replies use request_id
because id belongs to the RPC envelope. Helpers are bounded/owned; clipboard input
uses anonymous memory rather than temporary disk storage. Fork changes are listed
in each fork's FORK.md, with required file notices.

Not manually verified: real desktop clipboard ownership/browser launch,
keyboard/hover/resize interactions or live SSH home inheritance. Those external
services were not invoked by the automated gate. Stopped at the requested gate.
