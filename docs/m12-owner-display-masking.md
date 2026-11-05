# Ordinary typing and secret prefixes

Cause: the owner window reused the strict agent projection, which paints withheld
prefix bytes as stars. Even `Cla` became `Cl*` because `a` could start the
case-insensitive Bearer header. A known owner-capability prefix such as `d` can
similarly make `Claud` display as `Clau*`; no complete secret was recognized.

Lead-approved policy: show unconfirmed prefixes only in the owner's live display.
A complete built-in header masks its body immediately; complete known literals
remain masked. Agent streams/screen/history, copy/search and command anchors keep
the old strict withholding. Only owner snapshot paths call `display_lines`.

Checks:
- `cargo test -p clanky-core --lib typed_claude_stays_readable`: the real terminal
  input grows from C to Claude; expected owner text is identical at every step.
  Before the fix it failed at Cla -> Cl*. After the fix it passed. The same test
  keeps strict agent Claude -> Clau** / withheld Clau with a synthetic overlapping
  capability, and checks a short header-complete token and a complete known value.
- `scripts/gate-m6-masking.sh`: PASS, unchanged strict-stream expectations plus
  owner-display coverage for all existing generated complete-secret fixtures.
- `cargo test -p clanky-core --lib masked_command_anchors`: PASS.

No actual credential values were inspected or printed. The full workspace and GUI
were not retested. There is no new dependency or change to agent masking semantics.
